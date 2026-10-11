//! Avaliação estática e limitada de JavaScript/TypeScript das extensões Vortex.
//!
//! Nenhum código das extensões é executado. O avaliador resolve apenas literais,
//! constantes, objetos, arrays, `path.join`, templates e retornos simples de
//! funções. Qualquer outra expressão vira `Val::Unknown` com um rótulo legível.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_span::{GetSpan, Span};
use oxc_syntax::operator::{BinaryOperator, LogicalOperator};
use oxc_syntax::scope::ScopeFlags;

const MAX_DEPTH: usize = 24;

/// Estende a vida útil de uma referência a um nó da arena.
///
/// Os nós pertencem ao `Allocator` da extensão, que vive mais que todo o
/// `Ctx`; o `Visit` do oxc apenas não expõe essa vida útil na assinatura.
fn arena<'a, T>(node: &T) -> &'a T {
    // SAFETY: todos os nós vêm da mesma arena imutável, mantida viva pelo
    // chamador até o fim da extração (ver `extract::extract_extension`).
    unsafe { &*(node as *const T) }
}

#[derive(Clone, Copy)]
pub enum FnRef<'a> {
    Arrow(&'a ArrowFunctionExpression<'a>),
    Func(&'a Function<'a>),
}

impl<'a> FnRef<'a> {
    pub fn body(&self) -> Option<&'a FunctionBody<'a>> {
        match self {
            FnRef::Arrow(a) => match &a.body {
                ArrowFunctionBody::FunctionBody(b) => Some(b),
                _ => None,
            },
            FnRef::Func(f) => f.body.as_deref(),
        }
    }

    pub fn param_name(&self, index: usize) -> Option<&'a str> {
        let params = match self {
            FnRef::Arrow(a) => &a.params,
            FnRef::Func(f) => &f.params,
        };
        params.items.get(index).and_then(|p| match &p.pattern {
            BindingPattern::BindingIdentifier(id) => Some(id.name.as_str()),
            _ => None,
        })
    }

    /// Expressão de retorno de uma arrow sem bloco (`() => X`).
    fn expression_body(&self) -> Option<&'a Expression<'a>> {
        match self {
            FnRef::Arrow(a) => a.body.as_expression(),
            FnRef::Func(_) => None,
        }
    }

}

#[derive(Clone)]
pub enum Val<'a> {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    Undef,
    Arr(Vec<Val<'a>>),
    Obj(Vec<(String, Val<'a>)>),
    Fn(FnRef<'a>),
    Class(&'a Class<'a>),
    New(&'a NewExpression<'a>),
    Unknown(String),
}

impl<'a> Val<'a> {
    pub fn get(&self, key: &str) -> Option<&Val<'a>> {
        match self {
            Val::Obj(fields) => fields.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }


    /// Valor escalar como texto (números de IDs de loja incluídos).
    pub fn scalar_text(&self) -> Option<String> {
        match self {
            Val::Str(s) => Some(s.clone()),
            Val::Num(n) if n.fract() == 0.0 => Some(format!("{}", *n as i64)),
            Val::Num(n) => Some(n.to_string()),
            _ => None,
        }
    }

    fn as_str_like(&self) -> bool {
        matches!(self, Val::Str(_))
    }

    pub fn is_unknown(&self) -> bool {
        matches!(self, Val::Unknown(_))
    }

    pub fn is_nullish(&self) -> bool {
        matches!(self, Val::Null | Val::Undef)
    }

    /// Lista de textos de um array (ou de um escalar isolado).
    pub fn strings(&self) -> Vec<String> {
        match self {
            Val::Arr(items) => items.iter().flat_map(|v| v.strings()).collect(),
            other => other.scalar_text().into_iter().collect(),
        }
    }
}

/// Tabela de símbolos e chamadas de uma extensão (todos os arquivos juntos).
#[derive(Default)]
pub struct Ctx<'a> {
    /// Todas as declarações de cada nome; as do nível superior vêm primeiro.
    pub consts: HashMap<String, Vec<&'a Expression<'a>>>,
    pub funcs: HashMap<String, FnRef<'a>>,
    pub classes: HashMap<String, &'a Class<'a>>,
    /// Chamadas com o índice do arquivo de origem (spans são por arquivo).
    pub calls: Vec<(usize, &'a CallExpression<'a>)>,
    /// Laços `X.forEach(cb)` / `Object.keys(X).forEach(cb)`.
    pub loops: Vec<LoopCtx<'a>>,
    /// Valores temporários de variáveis de laço durante uma avaliação.
    pub env: RefCell<HashMap<String, Val<'a>>>,
    file: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LoopKind {
    Keys,
    Values,
    Items,
}

pub struct LoopCtx<'a> {
    pub file: usize,
    pub span: Span,
    pub param: String,
    pub source: &'a Expression<'a>,
    pub kind: LoopKind,
}

impl<'a> Ctx<'a> {
    /// Laço mais interno que contém a chamada.
    pub fn enclosing_loop(&self, file: usize, call: &CallExpression<'_>) -> Option<&LoopCtx<'a>> {
        let span = call.span;
        self.loops
            .iter()
            .filter(|l| l.file == file && l.span.start <= span.start && span.end <= l.span.end)
            .min_by_key(|l| l.span.end - l.span.start)
    }

    /// Itens percorridos por um laço, quando resolvíveis.
    pub fn loop_items(&self, l: &LoopCtx<'a>) -> Vec<Val<'a>> {
        match (l.kind, self.eval(l.source)) {
            (LoopKind::Keys, Val::Obj(fields)) => fields.into_iter().map(|(k, _)| Val::Str(k)).collect(),
            (LoopKind::Values, Val::Obj(fields)) => fields.into_iter().map(|(_, v)| v).collect(),
            (LoopKind::Items, Val::Arr(items)) => items,
            _ => Vec::new(),
        }
    }

    /// Registra declarações do nível superior primeiro, para que nomes
    /// sombreados dentro de funções não substituam as constantes do módulo.
    pub fn add_program(&mut self, file: usize, program: &'a Program<'a>) {
        self.file = file;
        for stmt in &program.body {
            self.add_top_statement(stmt);
        }
        let mut collector = Collector { ctx: self };
        collector.visit_program(program);
    }

    fn add_top_statement(&mut self, stmt: &'a Statement<'a>) {
        match stmt {
            Statement::VariableDeclaration(decl) => self.add_declarators(decl),
            Statement::FunctionDeclaration(f) => self.add_function(f),
            Statement::ClassDeclaration(c) => self.add_class(c),
            Statement::ExportDeclaration(e) => match &e.declaration {
                Declaration::VariableDeclaration(decl) => self.add_declarators(decl),
                Declaration::FunctionDeclaration(f) => self.add_function(f),
                Declaration::ClassDeclaration(c) => self.add_class(c),
                _ => {}
            },
            _ => {}
        }
    }

    fn add_declarators(&mut self, decl: &'a VariableDeclaration<'a>) {
        for d in &decl.declarations {
            if let (BindingPattern::BindingIdentifier(id), Some(init)) = (&d.id, &d.init) {
                push_decl(&mut self.consts, id.name.as_str(), init);
            }
        }
    }

    fn add_function(&mut self, f: &'a Function<'a>) {
        if let Some(id) = &f.id {
            self.funcs.entry(id.name.as_str().to_string()).or_insert(FnRef::Func(f));
        }
    }

    fn add_class(&mut self, c: &'a Class<'a>) {
        if let Some(id) = &c.id {
            self.classes.entry(id.name.as_str().to_string()).or_insert(c);
        }
    }

    pub fn eval(&self, expr: &'a Expression<'a>) -> Val<'a> {
        self.eval_depth(expr, 0, &mut HashSet::new())
    }

    fn eval_depth(&self, expr: &'a Expression<'a>, depth: usize, seen: &mut HashSet<String>) -> Val<'a> {
        if depth > MAX_DEPTH {
            return Val::Unknown(label(expr));
        }
        let next = depth + 1;
        match expr {
            Expression::StringLiteral(s) => Val::Str(s.value.as_str().to_string()),
            Expression::NumericLiteral(n) => Val::Num(n.value),
            Expression::BooleanLiteral(b) => Val::Bool(b.value),
            Expression::NullLiteral(_) => Val::Null,
            Expression::RegExpLiteral(r) => Val::Str(r.regex.pattern.text.as_str().to_string()),
            Expression::TemplateLiteral(t) => {
                let mut out = String::new();
                for (i, quasi) in t.quasis.iter().enumerate() {
                    let part = quasi.value.cooked.as_ref().map(|c| c.as_str()).unwrap_or(quasi.value.raw.as_str());
                    out.push_str(part);
                    if let Some(e) = t.expressions.get(i) {
                        match self.eval_depth(e, next, seen).scalar_text() {
                            Some(s) => out.push_str(&s),
                            None => out.push_str(&format!("{{?{}}}", label(e))),
                        }
                    }
                }
                Val::Str(out)
            }
            Expression::Identifier(id) => self.resolve_name(id.name.as_str(), next, seen),
            Expression::ArrayExpression(arr) => {
                let mut items = Vec::new();
                for el in &arr.elements {
                    match el {
                        ArrayExpressionElement::SpreadElement(s) => match self.eval_depth(&s.argument, next, seen) {
                            Val::Arr(inner) => items.extend(inner),
                            other => items.push(other),
                        },
                        ArrayExpressionElement::Elision(_) => items.push(Val::Undef),
                        other => {
                            if let Some(e) = other.as_expression() {
                                items.push(self.eval_depth(arena(e), next, seen));
                            }
                        }
                    }
                }
                Val::Arr(items)
            }
            Expression::ObjectExpression(obj) => {
                let mut fields = Vec::new();
                for prop in &obj.properties {
                    match prop {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            let key = match p.key.static_name() {
                                Some(k) => k.to_string(),
                                None => match p.key.as_expression().map(|e| self.eval_depth(arena(e), next, seen)) {
                                    Some(v) => match v.scalar_text() {
                                        Some(k) => k,
                                        None => continue,
                                    },
                                    None => continue,
                                },
                            };
                            fields.push((key, self.eval_depth(&p.value, next, seen)));
                        }
                        ObjectPropertyKind::SpreadProperty(s) => {
                            if let Val::Obj(inner) = self.eval_depth(&s.argument, next, seen) {
                                fields.extend(inner);
                            }
                        }
                    }
                }
                Val::Obj(fields)
            }
            Expression::StaticMemberExpression(m) => self.member(&m.object, m.property.name.as_str(), expr, next, seen),
            Expression::ComputedMemberExpression(m) => {
                let key = self.eval_depth(&m.expression, next, seen);
                match (self.eval_depth(&m.object, next, seen), key) {
                    (Val::Arr(items), Val::Num(i)) if i >= 0.0 => items.into_iter().nth(i as usize).unwrap_or(Val::Undef),
                    (obj @ Val::Obj(_), k) => match k.scalar_text() {
                        Some(k) => obj.get(&k).cloned().unwrap_or(Val::Undef),
                        None => Val::Unknown(label(expr)),
                    },
                    _ => Val::Unknown(label(expr)),
                }
            }
            Expression::CallExpression(call) => self.call(call, expr, next, seen),
            Expression::BinaryExpression(b) if b.operator == BinaryOperator::Addition => {
                let left = self.eval_depth(&b.left, next, seen);
                let right = self.eval_depth(&b.right, next, seen);
                match (&left, &right) {
                    (Val::Num(l), Val::Num(r)) => Val::Num(l + r),
                    _ => match (left.scalar_text(), right.scalar_text()) {
                        (Some(l), Some(r)) => Val::Str(l + &r),
                        _ => Val::Unknown(label(expr)),
                    },
                }
            }
            Expression::LogicalExpression(l) => {
                let left = self.eval_depth(&l.left, next, seen);
                match l.operator {
                    LogicalOperator::Or | LogicalOperator::Coalesce
                        if left.is_unknown() || left.is_nullish() || matches!(left, Val::Bool(false)) =>
                    {
                        self.eval_depth(&l.right, next, seen)
                    }
                    LogicalOperator::Or | LogicalOperator::Coalesce => left,
                    LogicalOperator::And => Val::Unknown(label(expr)),
                }
            }
            Expression::ConditionalExpression(c) => {
                // Sem executar o teste: prefere o ramo resolvido; com os dois
                // resolvidos, usa o primeiro (caso padrão nos templates).
                let yes = self.eval_depth(&c.consequent, next, seen);
                let no = self.eval_depth(&c.alternate, next, seen);
                match (&yes, &no) {
                    (Val::Unknown(_), Val::Unknown(_)) => Val::Unknown(label(expr)),
                    (Val::Unknown(_), _) => no,
                    _ => yes,
                }
            }
            Expression::ParenthesizedExpression(p) => self.eval_depth(&p.expression, next, seen),
            Expression::TSAsExpression(t) => self.eval_depth(&t.expression, next, seen),
            Expression::TSSatisfiesExpression(t) => self.eval_depth(&t.expression, next, seen),
            Expression::TSNonNullExpression(t) => self.eval_depth(&t.expression, next, seen),
            Expression::AwaitExpression(a) => self.eval_depth(&a.argument, next, seen),
            Expression::ArrowFunctionExpression(a) => Val::Fn(FnRef::Arrow(a)),
            Expression::FunctionExpression(f) => Val::Fn(FnRef::Func(f)),
            Expression::ClassExpression(c) => Val::Class(c),
            Expression::NewExpression(n) => Val::New(n),
            _ => Val::Unknown(label(expr)),
        }
    }

    fn resolve_name(&self, name: &str, depth: usize, seen: &mut HashSet<String>) -> Val<'a> {
        if name == "undefined" {
            return Val::Undef;
        }
        if let Some(v) = self.env.borrow().get(name) {
            return v.clone();
        }
        if !seen.insert(name.to_string()) {
            return Val::Unknown(name.to_string());
        }
        let value = if let Some(decls) = self.consts.get(name) {
            // Sem análise de escopo: entre declarações homônimas, usa a
            // primeira que produz um valor útil.
            let mut first = None;
            let mut useful = None;
            for e in decls {
                let v = self.eval_depth(e, depth, seen);
                if !v.is_unknown() && !v.is_nullish() {
                    useful = Some(v);
                    break;
                }
                first.get_or_insert(v);
            }
            useful.or(first).unwrap_or(Val::Undef)
        } else if let Some(f) = self.funcs.get(name) {
            Val::Fn(*f)
        } else if let Some(c) = self.classes.get(name) {
            Val::Class(c)
        } else {
            Val::Unknown(name.to_string())
        };
        seen.remove(name);
        value
    }

    fn member(
        &self,
        object: &'a Expression<'a>,
        prop: &str,
        whole: &'a Expression<'a>,
        depth: usize,
        seen: &mut HashSet<String>,
    ) -> Val<'a> {
        let object_label = label(object);
        if object_label == "path" && prop == "sep" {
            return Val::Str("/".into());
        }
        if object_label == "process.env" {
            return Val::Str(format!("{{env:{prop}}}"));
        }
        match self.eval_depth(object, depth, seen) {
            obj @ Val::Obj(_) => obj.get(prop).cloned().unwrap_or(Val::Undef),
            Val::Arr(items) if prop == "length" => Val::Num(items.len() as f64),
            Val::Str(s) if prop == "length" => Val::Num(s.len() as f64),
            _ => Val::Unknown(label(whole)),
        }
    }

    fn call(&self, call: &'a CallExpression<'a>, whole: &'a Expression<'a>, depth: usize, seen: &mut HashSet<String>) -> Val<'a> {
        let callee = label(&call.callee);
        let args: Vec<Val<'a>> = call
            .arguments
            .iter()
            .filter_map(|a| a.as_expression())
            .map(|e| self.eval_depth(arena(e), depth, seen))
            .collect();
        let method = callee.rsplit('.').next().unwrap_or(&callee);
        let receiver = callee.rsplit_once('.').map(|(r, _)| r).unwrap_or("");

        if matches!(receiver, "path" | "path.posix" | "path.win32" | "nodePath" | "posix")
            && matches!(method, "join" | "resolve" | "normalize")
        {
            let parts: Vec<String> = args.iter().map(path_part).collect();
            return Val::Str(join_path(&parts));
        }
        if receiver.ends_with("path") && method == "basename" {
            if let Some(s) = args.first().and_then(|a| a.scalar_text()) {
                return Val::Str(s.rsplit(['/', '\\']).next().unwrap_or(&s).to_string());
            }
        }
        if receiver.ends_with("path") && method == "dirname" {
            if let Some(s) = args.first().and_then(|a| a.scalar_text()) {
                let normalized = s.replace('\\', "/");
                return Val::Str(normalized.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default());
            }
        }
        if method == "getVortexPath" {
            if let Some(kind) = args.first().and_then(|a| a.scalar_text()) {
                return Val::Str(format!("{{{kind}}}"));
            }
        }
        // `pathPattern(api, game, "{gamePath}/...")` e `template(str, vars)` apenas
        // substituem marcadores; o padrão já está no formato dos destinos.
        if matches!(method, "pathPattern" | "template" | "resolvePattern") {
            if let Some(pattern) = args.iter().rev().find(|a| a.as_str_like()) {
                return pattern.clone();
            }
        }
        if matches!(callee.as_str(), "parseInt" | "Number" | "String" | "Number.parseInt") {
            if let Some(v) = args.first().and_then(|a| a.scalar_text()) {
                return Val::Str(v);
            }
        }
        if method == "toString" {
            if let Expression::StaticMemberExpression(m) = &call.callee {
                if let Some(v) = self.eval_depth(&m.object, depth, seen).scalar_text() {
                    return Val::Str(v);
                }
            }
        }
        if callee == "os.homedir" {
            return Val::Str("{home}".into());
        }
        if let Some(rest) = callee.strip_suffix(".toLowerCase") {
            let _ = rest;
            if let Expression::StaticMemberExpression(m) = &call.callee {
                if let Some(s) = self.eval_depth(&m.object, depth, seen).scalar_text() {
                    return Val::Str(s.to_lowercase());
                }
            }
        }
        if method == "concat" {
            if let Expression::StaticMemberExpression(m) = &call.callee {
                match self.eval_depth(&m.object, depth, seen) {
                    Val::Arr(mut items) => {
                        for a in args {
                            match a {
                                Val::Arr(inner) => items.extend(inner),
                                other => items.push(other),
                            }
                        }
                        return Val::Arr(items);
                    }
                    Val::Str(mut s) => {
                        for a in &args {
                            s.push_str(&a.scalar_text().unwrap_or_default());
                        }
                        return Val::Str(s);
                    }
                    _ => {}
                }
            }
        }
        // Funções locais que retornam um valor fixo (ex.: `getModPath()`).
        if let Expression::Identifier(id) = &call.callee {
            if let Val::Fn(f) = self.resolve_name(id.name.as_str(), depth, seen) {
                if let Some(ret) = self.return_value_depth(f, depth, seen) {
                    if !ret.is_unknown() {
                        return ret;
                    }
                }
            }
        }
        Val::Unknown(label(whole))
    }

    /// Valor retornado por uma função, quando ele independe de execução.
    pub fn return_value(&self, f: FnRef<'a>) -> Option<Val<'a>> {
        self.return_value_depth(f, 0, &mut HashSet::new())
    }

    fn return_value_depth(&self, f: FnRef<'a>, depth: usize, seen: &mut HashSet<String>) -> Option<Val<'a>> {
        if depth > MAX_DEPTH {
            return None;
        }
        if let Some(e) = f.expression_body() {
            return Some(self.eval_depth(e, depth + 1, seen));
        }
        let body = f.body()?;
        let mut returns = Vec::new();
        collect_returns(&body.statements, &mut returns);
        let mut first = None;
        for r in returns {
            let v = self.eval_depth(r, depth + 1, seen);
            if !v.is_unknown() && !v.is_nullish() {
                return Some(v);
            }
            first.get_or_insert(v);
        }
        first
    }
}

fn push_decl<'a>(map: &mut HashMap<String, Vec<&'a Expression<'a>>>, name: &str, init: &'a Expression<'a>) {
    let list = map.entry(name.to_string()).or_default();
    if !list.iter().any(|e| std::ptr::eq(*e, init)) {
        list.push(init);
    }
}

/// Avalia uma constante pelo nome (primeira declaração útil).
impl<'a> Ctx<'a> {
    pub fn eval_name(&self, name: &str) -> Val<'a> {
        self.resolve_name(name, 0, &mut HashSet::new())
    }
}

fn collect_returns<'a>(stmts: &'a [Statement<'a>], out: &mut Vec<&'a Expression<'a>>) {
    for stmt in stmts {
        match stmt {
            Statement::ReturnStatement(r) => {
                if let Some(arg) = &r.argument {
                    out.push(arg);
                }
            }
            Statement::BlockStatement(b) => collect_returns(&b.body, out),
            Statement::IfStatement(i) => {
                collect_returns(std::slice::from_ref(&i.consequent), out);
                if let Some(alt) = &i.alternate {
                    collect_returns(std::slice::from_ref(alt), out);
                }
            }
            Statement::TryStatement(t) => collect_returns(&t.block.body, out),
            _ => {}
        }
    }
}

fn path_part(v: &Val<'_>) -> String {
    match v {
        Val::Unknown(l) => placeholder_for(l),
        other => other.scalar_text().unwrap_or_else(|| "{?}".into()),
    }
}

/// Converte expressões conhecidas do Vortex em marcadores de caminho.
pub fn placeholder_for(label: &str) -> String {
    let lower = label.to_ascii_lowercase();
    if lower.contains("discovery") && lower.ends_with(".path")
        || lower.ends_with("gamepath")
        || lower.contains("getdiscoverypath")
        || lower.contains("getgamepath")
        || lower == "game_path"
    {
        "{gamePath}".into()
    } else if lower.contains("documents") || lower.contains("mygames") {
        "{documents}".into()
    } else if lower.contains("localappdata") {
        "{localAppData}".into()
    } else if lower.contains("appdata") {
        "{appData}".into()
    } else {
        format!("{{?{label}}}")
    }
}

pub fn join_path(parts: &[String]) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for part in parts {
        for seg in part.split(['/', '\\']) {
            match seg {
                "" | "." => {}
                ".." => {
                    segments.pop();
                }
                s => segments.push(s),
            }
        }
    }
    segments.join("/")
}

/// Rótulo legível de uma expressão não resolvida (`a.b.c`, `fn(...)`).
pub fn label(expr: &Expression<'_>) -> String {
    match expr {
        Expression::Identifier(id) => id.name.as_str().to_string(),
        Expression::ThisExpression(_) => "this".into(),
        Expression::StaticMemberExpression(m) => format!("{}.{}", label(&m.object), m.property.name.as_str()),
        Expression::ComputedMemberExpression(m) => format!("{}[{}]", label(&m.object), label(&m.expression)),
        Expression::CallExpression(c) => format!("{}()", label(&c.callee)).replace("()()", "()"),
        Expression::ChainExpression(c) => match &c.expression {
            ChainElement::CallExpression(call) => format!("{}()", label(&call.callee)),
            ChainElement::StaticMemberExpression(m) => format!("{}.{}", label(&m.object), m.property.name.as_str()),
            _ => "<chain>".into(),
        },
        Expression::StringLiteral(s) => format!("{:?}", s.value.as_str()),
        Expression::NumericLiteral(n) => n.value.to_string(),
        Expression::ParenthesizedExpression(p) => label(&p.expression),
        Expression::AwaitExpression(a) => label(&a.argument),
        Expression::TSAsExpression(t) => label(&t.expression),
        Expression::ConditionalExpression(_) => "<condicional>".into(),
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => "<função>".into(),
        _ => "<expr>".into(),
    }
    // Remove o sufixo duplicado gerado para chamadas encadeadas.
    .replace("()().", "().")
}

/// Coleta declarações aninhadas e todas as chamadas de função.
struct Collector<'c, 'a> {
    ctx: &'c mut Ctx<'a>,
}

impl<'a> Visit<'a> for Collector<'_, 'a> {
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let (BindingPattern::BindingIdentifier(id), Some(init)) = (&it.id, &it.init) {
            push_decl(&mut self.ctx.consts, id.name.as_str(), arena(init));
        }
        walk::walk_variable_declarator(self, it);
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        if let Some(id) = &it.id {
            self.ctx.funcs.entry(id.name.as_str().to_string()).or_insert(FnRef::Func(arena(it)));
        }
        walk::walk_function(self, it, flags);
    }

    fn visit_class(&mut self, it: &Class<'a>) {
        if let Some(id) = &it.id {
            self.ctx.classes.entry(id.name.as_str().to_string()).or_insert(arena(it));
        }
        walk::walk_class(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        let file = self.ctx.file;
        self.ctx.calls.push((file, arena(it)));
        if let Expression::StaticMemberExpression(m) = &it.callee {
            let method = m.property.name.as_str();
            let callback = it.arguments.first().and_then(|a| a.as_expression());
            if matches!(method, "forEach" | "map") {
                if let Some(cb) = callback {
                    let param = match cb {
                        Expression::ArrowFunctionExpression(a) => FnRef::Arrow(arena(a)).param_name(0),
                        Expression::FunctionExpression(f) => FnRef::Func(arena(f)).param_name(0),
                        _ => None,
                    };
                    if let Some(param) = param {
                        let (kind, source): (LoopKind, &'a Expression<'a>) = match &m.object {
                            Expression::CallExpression(inner) if label(&inner.callee) == "Object.keys" => {
                                (LoopKind::Keys, arena(inner.arguments.first().and_then(|a| a.as_expression()).unwrap_or(&m.object)))
                            }
                            Expression::CallExpression(inner) if label(&inner.callee) == "Object.values" => {
                                (LoopKind::Values, arena(inner.arguments.first().and_then(|a| a.as_expression()).unwrap_or(&m.object)))
                            }
                            other => (LoopKind::Items, arena(other)),
                        };
                        self.ctx.loops.push(LoopCtx { file, span: cb.span(), param: param.to_string(), source, kind });
                    }
                }
            }
        }
        walk::walk_call_expression(self, it);
    }
}

/// Textos, identificadores e chamadas presentes em um trecho de código.
#[derive(Default)]
pub struct Signals<'a> {
    pub strings: Vec<String>,
    pub identifiers: Vec<String>,
    pub calls: Vec<&'a CallExpression<'a>>,
}

impl<'a> Visit<'a> for Signals<'a> {
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        self.strings.push(it.value.as_str().to_string());
    }

    fn visit_identifier_reference(&mut self, it: &IdentifierReference<'a>) {
        self.identifiers.push(it.name.as_str().to_string());
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        self.calls.push(arena(it));
        walk::walk_call_expression(self, it);
    }
}

impl<'a> Signals<'a> {
    pub fn of_fn(f: FnRef<'a>) -> Self {
        let mut s = Signals::default();
        match f {
            FnRef::Arrow(a) => s.visit_arrow_function_expression(a),
            FnRef::Func(func) => s.visit_function(func, ScopeFlags::Function),
        }
        s
    }
}
