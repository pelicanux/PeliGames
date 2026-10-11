//! Extração dos fatos declarativos de uma extensão de jogo do Vortex.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::Serialize;
use walkdir::WalkDir;

use crate::js::{join_path, placeholder_for, Ctx, FnRef, Signals, Val};

#[derive(Debug, Default, Serialize, Clone)]
pub struct StoreIds {
    pub steam: BTreeSet<String>,
    pub gog: BTreeSet<String>,
    pub epic: BTreeSet<String>,
    pub xbox: BTreeSet<String>,
    pub registry: BTreeSet<String>,
    /// IDs passados ao `GameStoreHelper` sem loja identificável.
    pub unclassified: BTreeSet<String>,
}

impl StoreIds {
    pub fn is_empty(&self) -> bool {
        self.steam.is_empty() && self.gog.is_empty() && self.epic.is_empty() && self.xbox.is_empty() && self.registry.is_empty()
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ModType {
    pub id: String,
    pub name: Option<String>,
    pub priority: Option<String>,
    /// Destino com marcadores: `{gamePath}`, `{documents}`, `{localAppData}`...
    pub target: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Installer {
    pub id: String,
    pub priority: Option<f64>,
    pub mod_type: Option<String>,
    /// Extensões de arquivo citadas no teste do instalador.
    pub extensions: BTreeSet<String>,
    /// Nomes de arquivos/pastas citados no teste do instalador.
    pub names: BTreeSet<String>,
    pub checks_fomod: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct Tool {
    pub id: Option<String>,
    pub name: Option<String>,
    pub executable: Option<String>,
}

#[derive(Debug, Default, Serialize, Clone)]
pub struct Features {
    pub setup: bool,
    pub setup_dialog: bool,
    pub requires_launcher: bool,
    pub load_order: bool,
    pub merge: bool,
    pub deploy_hooks: BTreeSet<String>,
    pub tests: usize,
    pub actions: usize,
    pub game_version_query: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct GameFacts {
    pub source: String,
    pub extension_dir: String,
    pub game_id: Option<String>,
    pub name: Option<String>,
    pub nexus_domain: Option<String>,
    pub family: Option<String>,
    pub stub_repository: Option<String>,
    pub executable: Option<String>,
    pub required_files: Vec<String>,
    pub mod_path: Option<String>,
    pub merge_mods: Option<bool>,
    pub stores: StoreIds,
    pub stop_patterns: Vec<String>,
    pub environment: Vec<(String, String)>,
    pub mod_types: Vec<ModType>,
    pub installers: Vec<Installer>,
    pub tools: Vec<Tool>,
    pub dependency_links: BTreeSet<String>,
    /// Constantes conhecidas dos templates (ex.: `SHIPPING_EXE`, `EPIC_CODE_NAME`).
    pub constants: std::collections::BTreeMap<String, String>,
    pub features: Features,
    pub unresolved: BTreeSet<String>,
    pub parse_errors: Vec<String>,
    pub classification: Classification,
}

impl GameFacts {
    /// Identificação única: fonte, pasta da extensão e ID do jogo.
    pub fn key(&self) -> String {
        match &self.game_id {
            Some(id) => format!("{}/{}#{id}", self.source, self.extension_dir),
            None => format!("{}/{}", self.source, self.extension_dir),
        }
    }
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Green,
    Yellow,
    Red,
}

#[derive(Debug, Serialize, Clone)]
pub struct Classification {
    pub level: Level,
    pub reasons: Vec<String>,
    /// Dados próprios do jogo resolvidos: ID, loja, executável e algum destino.
    /// O comportamento da família (instaladores, ordem, hooks) é avaliado à parte.
    pub game_data_complete: bool,
}

fn source_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            !(e.file_type().is_dir()
                && matches!(name.as_ref(), "node_modules" | "dist" | "build" | "out" | "__tests__" | "test" | "tests" | "__mocks__"))
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            let ext_ok = [".js", ".ts", ".tsx", ".jsx", ".mjs", ".cjs"].iter().any(|e| name.ends_with(e));
            ext_ok && !name.ends_with(".d.ts") && !name.contains(".test.") && !name.contains(".spec.") && !name.ends_with(".config.js")
        })
        .collect();
    // O arquivo principal primeiro: suas constantes têm precedência.
    files.sort_by_key(|p| {
        let name = p.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        (!name.starts_with("index."), p.components().count(), p.clone())
    });
    files
}

/// Uma extensão pode registrar vários jogos (ex.: KOTOR e KOTOR II).
pub fn extract_extension(source: &str, dir: &Path) -> Vec<GameFacts> {
    let allocator = Allocator::default();
    let mut ctx = Ctx::default();
    let mut parse_errors = Vec::new();
    let mut header = String::new();

    for (index, file) in source_files(dir).into_iter().enumerate() {
        let Ok(text) = fs::read_to_string(&file) else { continue };
        if header.is_empty() {
            header = text.chars().take(1500).collect();
        }
        let text: &str = allocator.alloc_str(&text);
        let source_type = SourceType::from_path(&file).unwrap_or_else(|_| SourceType::cjs());
        let ret = Parser::new(&allocator, text, source_type).parse();
        if ret.diagnostics.has_errors() {
            parse_errors.push(format!("{}: {} erro(s) de sintaxe", file.display(), ret.diagnostics.errors().count()));
        }
        let program: &Program = allocator.alloc(ret.program);
        ctx.add_program(index, program);
    }

    let mut facts = GameFacts {
        source: source.to_string(),
        extension_dir: dir.file_name().unwrap_or_default().to_string_lossy().to_string(),
        game_id: None,
        name: None,
        nexus_domain: None,
        family: header_field(&header, "Structure"),
        stub_repository: None,
        executable: None,
        required_files: Vec::new(),
        mod_path: None,
        merge_mods: None,
        stores: StoreIds::default(),
        stop_patterns: Vec::new(),
        environment: Vec::new(),
        mod_types: Vec::new(),
        installers: Vec::new(),
        tools: Vec::new(),
        dependency_links: BTreeSet::new(),
        constants: Default::default(),
        features: Features::default(),
        unresolved: BTreeSet::new(),
        parse_errors,
        classification: Classification { level: Level::Red, reasons: Vec::new(), game_data_complete: false },
    };

    let mut games = Vec::new();
    Extractor { ctx: &ctx, facts: &mut facts, games: &mut games }.run();
    let mut result = if games.is_empty() {
        vec![facts]
    } else {
        let single = games.len() == 1;
        games.into_iter().map(|g| merge_shared(g, &facts, single)).collect()
    };
    for f in &mut result {
        classify(f);
    }
    result
}

/// Completa um jogo registrado com o que a extensão declara para todos os
/// seus jogos (mod types, instaladores, ferramentas, recursos).
fn merge_shared(mut game: GameFacts, base: &GameFacts, single: bool) -> GameFacts {
    if single {
        game.game_id = game.game_id.or_else(|| base.game_id.clone());
        game.name = game.name.or_else(|| base.name.clone());
        game.nexus_domain = game.nexus_domain.or_else(|| base.nexus_domain.clone());
        game.executable = game.executable.or_else(|| base.executable.clone());
        game.mod_path = game.mod_path.or_else(|| base.mod_path.clone());
        game.merge_mods = game.merge_mods.or(base.merge_mods);
        for r in &base.required_files {
            push_unique(&mut game.required_files, r.clone());
        }
        let (g, b) = (&mut game.stores, &base.stores);
        g.steam.extend(b.steam.iter().cloned());
        g.gog.extend(b.gog.iter().cloned());
        g.epic.extend(b.epic.iter().cloned());
        g.xbox.extend(b.xbox.iter().cloned());
        g.registry.extend(b.registry.iter().cloned());
        g.unclassified.extend(b.unclassified.iter().cloned());
        let known: BTreeSet<String> = g.steam.iter().chain(&g.gog).chain(&g.epic).chain(&g.xbox).cloned().collect();
        g.unclassified.retain(|id| !known.contains(id));
        for p in &base.stop_patterns {
            push_unique(&mut game.stop_patterns, p.clone());
        }
        if game.environment.is_empty() {
            game.environment = base.environment.clone();
        }
    }
    game.family = base.family.clone();
    game.stub_repository = game.stub_repository.or_else(|| base.stub_repository.clone());
    for mt in &base.mod_types {
        if !game.mod_types.iter().any(|m| m.id == mt.id) {
            game.mod_types.push(mt.clone());
        }
    }
    game.installers.extend(base.installers.iter().cloned());
    if game.tools.is_empty() {
        game.tools = base.tools.clone();
    }
    game.dependency_links.extend(base.dependency_links.iter().cloned());
    for (k, v) in &base.constants {
        game.constants.entry(k.clone()).or_insert_with(|| v.clone());
    }
    let (gf, bf) = (&mut game.features, &base.features);
    gf.load_order |= bf.load_order;
    gf.merge |= bf.merge;
    gf.deploy_hooks.extend(bf.deploy_hooks.iter().cloned());
    gf.tests += bf.tests;
    gf.actions += bf.actions;
    game.unresolved.extend(base.unresolved.iter().cloned());
    game.parse_errors = base.parse_errors.clone();
    game
}

fn empty_like(f: &GameFacts) -> GameFacts {
    GameFacts {
        source: f.source.clone(),
        extension_dir: f.extension_dir.clone(),
        game_id: None,
        name: None,
        nexus_domain: None,
        family: f.family.clone(),
        stub_repository: None,
        executable: None,
        required_files: Vec::new(),
        mod_path: None,
        merge_mods: None,
        stores: StoreIds::default(),
        stop_patterns: Vec::new(),
        environment: Vec::new(),
        mod_types: Vec::new(),
        installers: Vec::new(),
        tools: Vec::new(),
        dependency_links: BTreeSet::new(),
        constants: Default::default(),
        features: Features::default(),
        unresolved: BTreeSet::new(),
        parse_errors: Vec::new(),
        classification: Classification { level: Level::Red, reasons: Vec::new(), game_data_complete: false },
    }
}

fn header_field(header: &str, field: &str) -> Option<String> {
    header.lines().find_map(|line| {
        let line = line.trim().trim_start_matches(['*', '/', ' ']);
        line.strip_prefix(field).and_then(|rest| rest.trim_start().strip_prefix(':')).map(|v| v.trim().to_string())
    })
}

struct Extractor<'c, 'a> {
    ctx: &'c Ctx<'a>,
    /// Dados compartilhados da extensão (e do jogo, quando não há `registerGame`).
    facts: &'c mut GameFacts,
    /// Um item por chamada efetiva de `registerGame`.
    games: &'c mut Vec<GameFacts>,
}

impl<'a> Extractor<'_, 'a> {
    fn run(&mut self) {
        let ctx = self.ctx;
        for &(file, call) in &ctx.calls {
            let callee = crate::js::label(&call.callee);
            let method = callee.rsplit('.').next().unwrap_or(&callee).to_string();
            let args: Vec<&'a Expression<'a>> = call.arguments.iter().filter_map(|a| a.as_expression()).collect();
            match method.as_str() {
                "registerGame" => {
                    let Some(arg) = args.first() else { continue };
                    // Em laços, avalia o registro uma vez por item.
                    let iterations: Vec<Option<(String, Val<'a>)>> = match ctx.enclosing_loop(file, call) {
                        Some(l) => {
                            let items = ctx.loop_items(l);
                            if items.is_empty() { vec![None] } else { items.into_iter().map(|v| Some((l.param.clone(), v))).collect() }
                        }
                        None => vec![None],
                    };
                    for binding in iterations {
                        if let Some((name, value)) = &binding {
                            ctx.env.borrow_mut().insert(name.clone(), value.clone());
                        }
                        let mut game = empty_like(self.facts);
                        std::mem::swap(self.facts, &mut game);
                        let obj = self.game_object(ctx.eval(arg));
                        self.read_game(&obj);
                        std::mem::swap(self.facts, &mut game);
                        if let Some((name, _)) = &binding {
                            ctx.env.borrow_mut().remove(name);
                        }
                        if !self.games.iter().any(|g| g.game_id.is_some() && g.game_id == game.game_id) {
                            self.games.push(game);
                        }
                    }
                }
                "registerGameStub" => self.read_stub(&args),
                "registerModType" => self.read_mod_type(&args),
                "registerInstaller" => self.read_installer(&args),
                "registerLoadOrder" | "registerLoadOrderPage" => self.facts.features.load_order = true,
                "registerMerge" => self.facts.features.merge = true,
                "registerTest" => self.facts.features.tests += 1,
                "registerAction" => self.facts.features.actions += 1,
                "onAsync" | "on" | "onStateChange" => {
                    if let Some(name) = args.first().and_then(|a| ctx.eval(a).scalar_text()) {
                        if matches!(name.as_str(), "did-deploy" | "will-deploy" | "did-purge" | "will-purge" | "did-install-mod") {
                            self.facts.features.deploy_hooks.insert(name);
                        }
                    }
                }
                _ => {}
            }
        }
        self.read_game_spec();
        self.read_named_store_constants();
        self.collect_dependency_links();
        for name in [
            "SHIPPING_EXE", "EPIC_CODE_NAME", "EXEC_FOLDER_DEFAULT", "STEAMAPP_ID",
            // Template Unity (BepInEx/MelonLoader).
            "DATA_FOLDER_DEFAULT", "EXEC", "recommendedLoader", "loaderChoice", "isXna", "hasCustomLoader", "ARCH", "BEPINEX_BUILD",
            // Template Basic.
            "MOD_PATH", "MOD_EXTS", "DATA_FOLDER", "needsModInstaller", "rootInstaller", "fallbackInstaller", "binariesInstaller",
        ] {
            if self.ctx.consts.contains_key(name) {
                let value = match self.ctx.eval_name(name) {
                    Val::Bool(b) => Some(b.to_string()),
                    // Lists (e.g. MOD_EXTS) are kept as comma-separated text.
                    Val::Arr(items) => Some(items.iter().filter_map(|v| v.scalar_text()).collect::<Vec<_>>().join(",")).filter(|v| !v.is_empty()),
                    other => other.scalar_text(),
                };
                if let Some(v) = value.filter(|v| !v.contains("{?")) {
                    self.facts.constants.insert(name.to_string(), v.replace('\\', "/"));
                }
            }
        }
    }

    /// Converte `new Classe(context)` em um objeto com campos e métodos.
    fn game_object(&self, value: Val<'a>) -> Val<'a> {
        let class = match &value {
            Val::New(n) => match self.ctx.eval(&n.callee) {
                Val::Class(c) => Some(c),
                _ => None,
            },
            Val::Class(c) => Some(*c),
            _ => None,
        };
        let Some(class) = class else { return value };
        let mut fields = Vec::new();
        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(m) => {
                    let Some(key) = m.key.static_name() else { continue };
                    if key == "constructor" {
                        if let Some(body) = &m.value.body {
                            self.this_assignments(&body.statements, &mut fields);
                        }
                    } else {
                        fields.push((key.to_string(), Val::Fn(FnRef::Func(&m.value))));
                    }
                }
                ClassElement::PropertyDefinition(p) => {
                    if let (Some(key), Some(value)) = (p.key.static_name(), &p.value) {
                        fields.push((key.to_string(), self.ctx.eval(value)));
                    }
                }
                _ => {}
            }
        }
        Val::Obj(fields)
    }

    fn this_assignments(&self, stmts: &'a [Statement<'a>], fields: &mut Vec<(String, Val<'a>)>) {
        for stmt in stmts {
            let Statement::ExpressionStatement(s) = stmt else { continue };
            let Expression::AssignmentExpression(a) = &s.expression else { continue };
            if let AssignmentTarget::StaticMemberExpression(m) = &a.left {
                if matches!(m.object, Expression::ThisExpression(_)) {
                    fields.push((m.property.name.as_str().to_string(), self.ctx.eval(&a.right)));
                }
            }
        }
    }

    fn value_or_return(&self, v: Option<&Val<'a>>) -> Option<Val<'a>> {
        match v? {
            Val::Fn(f) => self.ctx.return_value(*f),
            other => Some(other.clone()),
        }
    }

    fn read_game(&mut self, game: &Val<'a>) {
        let f = &mut *self.facts;
        if let Some(id) = game.get("id").and_then(|v| v.scalar_text()) {
            f.game_id.get_or_insert(id);
        }
        if let Some(name) = game.get("name").and_then(|v| v.scalar_text()) {
            f.name.get_or_insert(name);
        }
        if let Some(Val::Bool(b)) = game.get("mergeMods") {
            f.merge_mods = Some(*b);
        }
        if let Some(exe) = self.value_or_return(game.get("executable")) {
            match exe.scalar_text() {
                Some(e) => {
                    self.facts.executable.get_or_insert(e.replace('\\', "/"));
                }
                None => {
                    self.facts.unresolved.insert(format!("executable: {}", describe(&exe)));
                }
            }
        }
        if let Some(req) = self.value_or_return(game.get("requiredFiles")) {
            for r in req.strings() {
                push_unique(&mut self.facts.required_files, r.replace('\\', "/"));
            }
            if let Val::Arr(items) = &req {
                for item in items.iter().filter(|v| v.is_unknown()) {
                    self.facts.unresolved.insert(format!("requiredFiles: {}", describe(item)));
                }
            }
        }
        if let Some(mp) = self.value_or_return(game.get("queryModPath")).or_else(|| game.get("modPath").cloned()) {
            let rendered = render_path(&mp);
            if rendered.contains("{?") {
                self.facts.unresolved.insert(format!("queryModPath: {rendered}"));
            }
            self.facts.mod_path.get_or_insert(rendered);
        }
        if let Some(details) = game.get("details") {
            self.read_store_fields(details);
            if let Some(domain) = details.get("nexusPageId").and_then(|v| v.scalar_text()) {
                self.facts.nexus_domain.get_or_insert(domain);
            }
            if let Some(stop) = details.get("stopPatterns") {
                for s in stop.strings() {
                    push_unique(&mut self.facts.stop_patterns, s);
                }
            }
        }
        if let Some(Val::Obj(env)) = game.get("environment") {
            for (k, v) in env {
                if let Some(v) = v.scalar_text() {
                    self.facts.environment.push((k.clone(), v));
                }
            }
        }
        if let Some(query_args) = game.get("queryArgs") {
            self.read_query_args(query_args);
        }
        if let Some(Val::Fn(query)) = game.get("queryPath") {
            self.read_query_path(*query);
        }
        if let Some(Val::Arr(tools)) = game.get("supportedTools") {
            for t in tools {
                let exe = self.value_or_return(t.get("executable")).and_then(|v| v.scalar_text());
                self.facts.tools.push(Tool {
                    id: t.get("id").and_then(|v| v.scalar_text()),
                    name: t.get("name").and_then(|v| v.scalar_text()),
                    executable: exe.map(|e| e.replace('\\', "/")),
                });
            }
        }
        if let Some(Val::Fn(setup)) = game.get("setup") {
            self.facts.features.setup = true;
            let signals = Signals::of_fn(*setup);
            if signals.identifiers.iter().any(|i| i == "showDialog") || signals.calls.iter().any(|c| crate::js::label(&c.callee).ends_with("showDialog")) {
                self.facts.features.setup_dialog = true;
            }
        }
        self.facts.features.requires_launcher |= game.get("requiresLauncher").is_some();
        self.facts.features.game_version_query |= game.get("getGameVersion").is_some();
    }

    fn read_store_fields(&mut self, obj: &Val<'a>) {
        let s = &mut self.facts.stores;
        for (key, store) in [("steamAppId", 0), ("gogAppId", 1), ("epicAppId", 2), ("xboxAppId", 3), ("msAppId", 3)] {
            let Some(v) = obj.get(key) else { continue };
            let set = match store {
                0 => &mut s.steam,
                1 => &mut s.gog,
                2 => &mut s.epic,
                _ => &mut s.xbox,
            };
            set.extend(v.strings().into_iter().filter(|x| valid_id(x)));
        }
    }

    fn read_query_args(&mut self, args: &Val<'a>) {
        let s = &mut self.facts.stores;
        for (key, set) in [
            ("steam", &mut s.steam),
            ("gog", &mut s.gog),
            ("epic", &mut s.epic),
            ("xbox", &mut s.xbox),
            ("registry", &mut s.registry),
        ] {
            if let Some(Val::Arr(entries)) = args.get(key) {
                for e in entries {
                    if let Some(id) = e.get("id").and_then(|v| v.scalar_text()).filter(|x| valid_id(x)) {
                        set.insert(id);
                    }
                }
            }
        }
    }

    fn read_query_path(&mut self, f: FnRef<'a>) {
        let signals = Signals::of_fn(f);
        let mut queue: Vec<&'a CallExpression<'a>> = signals.calls.clone();
        // Segue funções locais chamadas pelo queryPath (ex.: `findGame()`).
        for name in &signals.identifiers {
            if let Some(inner) = self.ctx.funcs.get(name) {
                queue.extend(Signals::of_fn(*inner).calls);
            }
        }
        for call in queue {
            let callee = crate::js::label(&call.callee);
            let args: Vec<&'a Expression<'a>> = call.arguments.iter().filter_map(|a| a.as_expression()).collect();
            if callee.ends_with("findByAppId") {
                let ids = args.first().map(|a| self.ctx.eval(a).strings()).unwrap_or_default();
                let store = args.get(1).and_then(|a| self.ctx.eval(a).scalar_text());
                let s = &mut self.facts.stores;
                let target = match store.as_deref() {
                    Some("steam") => &mut s.steam,
                    Some("gog") => &mut s.gog,
                    Some("epic") => &mut s.epic,
                    Some("xbox") => &mut s.xbox,
                    _ => &mut s.unclassified,
                };
                target.extend(ids.into_iter().filter(|x| valid_id(x)));
            } else if callee.ends_with("RegGetValue") {
                let parts: Vec<String> = args.iter().take(2).filter_map(|a| self.ctx.eval(a).scalar_text()).collect();
                if parts.len() == 2 {
                    self.facts.stores.registry.insert(parts.join("\\"));
                }
            }
        }
    }

    /// Modelo `gameSpec` usado pelo template oficial e pelas extensões ChemBoy1.
    fn read_game_spec(&mut self) {
        let ctx = self.ctx;
        let mut names: Vec<&String> = ctx.consts.keys().collect();
        names.sort();
        for name in names {
            for expr in &ctx.consts[name] {
            if !matches!(expr, Expression::ObjectExpression(_)) {
                continue;
            }
            let spec = ctx.eval(expr);
            let (Some(game), Some(Val::Arr(mod_types))) = (spec.get("game"), spec.get("modTypes")) else { continue };
            self.read_game(game);
            if let Some(Val::Arr(ids)) = spec.get("discovery").and_then(|d| d.get("ids")) {
                let ids: Vec<String> = ids.iter().flat_map(|v| v.strings()).filter(|x| valid_id(x)).collect();
                self.facts.stores.unclassified.extend(ids);
            }
            if let Some(Val::Arr(tools)) = spec.get("tools") {
                for t in tools {
                    self.facts.tools.push(Tool {
                        id: t.get("id").and_then(|v| v.scalar_text()),
                        name: t.get("name").and_then(|v| v.scalar_text()),
                        executable: t.get("executable").and_then(|v| v.scalar_text()).map(|e| e.replace('\\', "/")),
                    });
                }
            }
            for mt in mod_types {
                let Some(id) = mt.get("id").and_then(|v| v.scalar_text()) else { continue };
                self.upsert_mod_type(ModType {
                    id,
                    name: mt.get("name").and_then(|v| v.scalar_text()),
                    priority: mt.get("priority").and_then(|v| v.scalar_text()),
                    target: mt.get("targetPath").map(render_path),
                });
            }
            }
        }
    }

    /// Constantes convencionais (`STEAMAPP_ID`, `EPICAPP_ID`...), comuns no ChemBoy1.
    fn read_named_store_constants(&mut self) {
        for name in self.ctx.consts.keys() {
            let upper = name.to_ascii_uppercase();
            if !upper.ends_with("_ID") && !upper.ends_with("APPID") {
                continue;
            }
            let ids: Vec<String> = self.ctx.eval_name(name).strings().into_iter().filter(|x| valid_id(x)).collect();
            if ids.is_empty() {
                continue;
            }
            let s = &mut self.facts.stores;
            let target = if upper.starts_with("STEAM") {
                &mut s.steam
            } else if upper.starts_with("GOG") {
                &mut s.gog
            } else if upper.starts_with("EPIC") {
                &mut s.epic
            } else if upper.starts_with("XBOX") || upper.starts_with("MS_") || upper.starts_with("UWP") {
                &mut s.xbox
            } else {
                continue;
            };
            target.extend(ids);
        }
        let s = &mut self.facts.stores;
        let known: BTreeSet<String> = s.steam.iter().chain(&s.gog).chain(&s.epic).chain(&s.xbox).cloned().collect();
        s.unclassified.retain(|id| !known.contains(id));
    }

    fn read_stub(&mut self, args: &[&'a Expression<'a>]) {
        for a in args {
            let v = self.ctx.eval(a);
            for key in ["repo", "repository", "url", "source"] {
                if let Some(r) = v.get(key).and_then(|x| x.scalar_text()) {
                    self.facts.stub_repository.get_or_insert(r);
                }
            }
            if let Some(game) = v.get("game") {
                self.read_game(game);
            } else if v.get("id").is_some() {
                self.read_game(&v);
            }
            if let Val::Str(s) = &v {
                if s.contains("github.com") || s.contains('/') {
                    self.facts.stub_repository.get_or_insert(s.clone());
                }
            }
        }
    }

    fn read_mod_type(&mut self, args: &[&'a Expression<'a>]) {
        let Some(id) = args.first().and_then(|a| self.ctx.eval(a).scalar_text()) else {
            // Registro em laço (`gameSpec.modTypes.forEach`): coberto por `read_game_spec`.
            return;
        };
        let priority = args.get(1).and_then(|a| self.ctx.eval(a).scalar_text());
        let target = args.get(3).map(|a| match self.ctx.eval(a) {
            Val::Fn(f) => match self.ctx.return_value(f) {
                Some(v) => render_path(&v),
                None => "{?getPath}".to_string(),
            },
            other => render_path(&other),
        });
        let name = args.get(5).and_then(|a| self.ctx.eval(a).get("name").and_then(|n| n.scalar_text()));
        if let Some(t) = target.as_ref().filter(|t| t.contains("{?")) {
            self.facts.unresolved.insert(format!("modType {id}: {t}"));
        }
        self.upsert_mod_type(ModType { id, name, priority, target });
    }

    fn upsert_mod_type(&mut self, mt: ModType) {
        if let Some(existing) = self.facts.mod_types.iter_mut().find(|m| m.id == mt.id) {
            let better = |a: &Option<String>| a.as_ref().is_some_and(|t| !t.contains("{?"));
            if !better(&existing.target) && better(&mt.target) {
                existing.target = mt.target;
            }
            existing.name = existing.name.take().or(mt.name);
            existing.priority = existing.priority.take().or(mt.priority);
        } else {
            self.facts.mod_types.push(mt);
        }
    }

    fn read_installer(&mut self, args: &[&'a Expression<'a>]) {
        let ctx = self.ctx;
        let Some(id) = args.first().and_then(|a| ctx.eval(a).scalar_text()) else { return };
        let priority = args.get(1).and_then(|a| match ctx.eval(a) {
            Val::Num(n) => Some(n),
            _ => None,
        });
        let mut extensions = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut checks_fomod = false;
        if let Some(Val::Fn(test)) = args.get(2).map(|a| ctx.eval(a)) {
            let mut texts = Vec::new();
            let signals = Signals::of_fn(test);
            texts.extend(signals.strings.iter().cloned());
            let mut idents = signals.identifiers.clone();
            // Um nível de funções auxiliares chamadas pelo teste.
            for name in &signals.identifiers {
                if let Some(inner) = ctx.funcs.get(name) {
                    let s = Signals::of_fn(*inner);
                    texts.extend(s.strings);
                    idents.extend(s.identifiers);
                }
            }
            for ident in idents {
                if ctx.consts.contains_key(&ident) {
                    texts.extend(ctx.eval_name(&ident).strings());
                }
            }
            for t in texts {
                let lower = t.to_lowercase();
                if lower.contains("fomod") || lower.contains("moduleconfig") {
                    checks_fomod = true;
                    continue;
                }
                if lower.starts_with('.') && lower.len() <= 12 && lower[1..].chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    extensions.insert(lower);
                } else if is_path_like(&t) || is_folder_word(&t) {
                    names.insert(t.replace('\\', "/"));
                }
            }
        }
        let mod_type = args.get(3).and_then(|a| match ctx.eval(a) {
            Val::Fn(install) => find_setmodtype(ctx, install),
            _ => None,
        });
        self.facts.installers.push(Installer { id, priority, mod_type, extensions, names, checks_fomod });
    }

    fn collect_dependency_links(&mut self) {
        for name in self.ctx.consts.keys() {
            for s in self.ctx.eval_name(name).strings() {
                let lower = s.to_lowercase();
                if (lower.contains("nexusmods.com/") && lower.contains("/mods/")) || (lower.contains("github.com/") && lower.contains("releases")) {
                    self.facts.dependency_links.insert(s);
                }
            }
        }
    }
}

fn find_setmodtype<'a>(ctx: &Ctx<'a>, install: FnRef<'a>) -> Option<String> {
    let signals = Signals::of_fn(install);
    let mut candidates = Vec::new();
    // `{ type: 'setmodtype', value: X }`: o valor costuma vir de uma constante.
    if signals.strings.iter().any(|s| s == "setmodtype") {
        for ident in &signals.identifiers {
            if ident.ends_with("_ID") || ident.ends_with("TYPE") {
                if let Some(v) = ctx.consts.contains_key(ident).then(|| ctx.eval_name(ident)).and_then(|v| v.scalar_text()) {
                    candidates.push(v);
                }
            }
        }
    }
    candidates.into_iter().next()
}

fn is_path_like(s: &str) -> bool {
    if s.is_empty() || s.len() > 120 || s.contains(' ') && !s.contains('/') {
        return false;
    }
    let has_sep = s.contains('/') || s.contains('\\');
    let has_file_ext = s.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()));
    let camel_folder = s.chars().next().is_some_and(|c| c.is_ascii_uppercase()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') && s.len() >= 3;
    (has_sep || has_file_ext || camel_folder) && !s.starts_with("http") && !s.contains('{') && !s.contains('$')
}

/// Palavra em minúsculas usada como nome de pasta nos testes (`"binaries"`).
fn is_folder_word(s: &str) -> bool {
    const NOT_FOLDERS: &[&str] = &[
        "string", "function", "undefined", "object", "number", "boolean", "warn", "info", "debug", "error", "utf8", "utf-8",
        "copy", "mkdir", "setmodtype", "attribute", "generatefile", "supported", "type", "value", "true", "false", "null",
        "steam", "epic", "gog", "xbox", "default", "name", "path", "files", "file", "length", "then", "catch", "dir", "base",
    ];
    (3..=40).contains(&s.len())
        && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' || c == '~')
        && s.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c == '~')
        && !NOT_FOLDERS.contains(&s)
}

fn valid_id(s: &str) -> bool {
    !s.is_empty() && s != "XXX" && s != "null" && !s.contains(' ') && s.len() <= 120
}

fn push_unique(v: &mut Vec<String>, s: String) {
    if !s.is_empty() && !v.contains(&s) {
        v.push(s);
    }
}

pub fn render_path(v: &Val<'_>) -> String {
    match v {
        Val::Str(s) => {
            let normalized = join_path(&[s.clone()]);
            if s.starts_with('{') || normalized.is_empty() {
                if normalized.is_empty() { "{gamePath}".into() } else { normalized }
            } else {
                normalized
            }
        }
        Val::Unknown(l) => placeholder_for(l),
        Val::Undef | Val::Null => "{gamePath}".into(),
        other => format!("{{?{}}}", describe(other)),
    }
}

fn describe(v: &Val<'_>) -> String {
    match v {
        Val::Unknown(l) => l.clone(),
        Val::Fn(_) => "<função>".into(),
        Val::Obj(_) => "<objeto>".into(),
        Val::Arr(_) => "<array>".into(),
        Val::New(_) | Val::Class(_) => "<classe>".into(),
        other => other.scalar_text().unwrap_or_else(|| "<valor>".into()),
    }
}

fn classify(f: &mut GameFacts) {
    let mut red = Vec::new();
    let mut yellow = Vec::new();

    if f.game_id.is_none() {
        red.push("registro do jogo não resolvido".to_string());
    }
    if f.stub_repository.is_some() && f.mod_types.is_empty() && f.installers.is_empty() {
        red.push("stub: a extensão real está em outro repositório".to_string());
    }
    if f.features.merge {
        red.push("mescla de arquivos (registerMerge)".into());
    }
    let family = f.family.clone().unwrap_or_default().to_lowercase();
    if ["fluffy", "frosty", "anvil", "reloaded", "tfc", "mod installer", "3rd", "overstrike", "srmm"].iter().any(|k| family.contains(k)) {
        red.push(format!("delega a ferramenta externa ({})", f.family.clone().unwrap_or_default()));
    }
    if f.stores.is_empty() && f.stores.unclassified.is_empty() {
        yellow.push("sem ID de loja resolvido".into());
    }
    if f.executable.is_none() {
        yellow.push("executável não resolvido".into());
    }
    if f.mod_types.iter().any(|m| m.target.as_ref().is_none_or(|t| t.contains("{?"))) {
        yellow.push("destino de mod type não resolvido".into());
    }
    if f.mod_path.as_ref().is_some_and(|m| m.contains("{?")) {
        yellow.push("pasta padrão de mods não resolvida".into());
    }
    // Fallbacks aceitam qualquer pacote do jogo por definição.
    if f.installers.iter().any(|i| i.extensions.is_empty() && i.names.is_empty() && !i.id.ends_with("fallback")) {
        yellow.push("instalador com teste sem sinais estáticos".into());
    }
    if f.features.load_order {
        yellow.push("ordem de carregamento própria".into());
    }
    if f.features.setup_dialog {
        yellow.push("setup exibe diálogo (dependência ou ação manual)".into());
    }
    if !f.parse_errors.is_empty() {
        yellow.push("erros de sintaxe na leitura".into());
    }

    let resolved = |t: &String| !t.contains("{?");
    let game_data_complete = f.game_id.is_some()
        && (!f.stores.is_empty() || !f.stores.unclassified.is_empty())
        && f.executable.is_some()
        && (f.mod_path.as_ref().is_some_and(resolved) || f.mod_types.iter().any(|m| m.target.as_ref().is_some_and(resolved)));
    let (level, reasons) = if !red.is_empty() {
        red.extend(yellow);
        (Level::Red, red)
    } else if !yellow.is_empty() {
        (Level::Yellow, yellow)
    } else {
        (Level::Green, vec!["somente dados declarativos resolvidos".into()])
    };
    f.classification = Classification { level, reasons, game_data_complete };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn extension(files: &[(&str, &str)]) -> Vec<GameFacts> {
        let dir = std::env::temp_dir().join(format!(
            "peligames-vortex-import-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let ext = dir.join("game-test");
        for (name, text) in files {
            let path = ext.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        let facts = extract_extension("teste", &ext);
        fs::remove_dir_all(&dir).unwrap();
        facts
    }

    #[test]
    fn class_registration_with_constants_and_steam_query() {
        let facts = extension(&[(
            "src/index.js",
            r#"
const path = require("path");
const { util } = require("vortex-api");
const STEAM_ID = "211420";
class DarkSouls {
  constructor(context) {
    this.id = "darksouls";
    this.name = "Dark Souls";
    this.mergeMods = true;
    this.details = { steamAppId: 211420 };
    this.requiredFiles = ["DATA/DARKSOULS.exe"];
  }
  queryPath() { return util.GameStoreHelper.findByAppId(STEAM_ID, "steam").then((g) => g.gamePath); }
  queryModPath() { return path.join("DATA", "dsfix", "tex_override"); }
  executable() { return path.join("DATA", "DARKSOULS.exe"); }
}
function main(context) { context.registerGame(new DarkSouls(context)); return true; }
module.exports = { default: main };
"#,
        )]);
        assert_eq!(facts.len(), 1);
        let f = &facts[0];
        assert_eq!(f.game_id.as_deref(), Some("darksouls"));
        assert_eq!(f.executable.as_deref(), Some("DATA/DARKSOULS.exe"));
        assert_eq!(f.mod_path.as_deref(), Some("DATA/dsfix/tex_override"));
        assert_eq!(f.required_files, vec!["DATA/DARKSOULS.exe"]);
        assert!(f.stores.steam.contains("211420"));
        assert!(f.classification.game_data_complete);
    }

    #[test]
    fn game_spec_template_with_mod_types_and_installers() {
        let facts = extension(&[(
            "index.js",
            r#"
const path = require("path");
const GAME_ID = "stellarblade";
const STEAMAPP_ID = "3489700";
const EPICAPP_ID = "abc123";
const EXEC = "SB.exe";
const PAK_EXT = ".pak";
const LOGICMODS_PATH = path.join("SB", "Content", "Paks", "LogicMods");
const LOGICMODS_ID = `${GAME_ID}-logicmods`;
const spec = {
  game: { id: GAME_ID, name: "Stellar Blade", executable: EXEC, requiredFiles: [EXEC], modPath: path.join("SB", "Content", "Paks", "~mods"), details: { steamAppId: +STEAMAPP_ID } },
  modTypes: [{ id: LOGICMODS_ID, name: "LogicMods", priority: "high", targetPath: path.join("{gamePath}", LOGICMODS_PATH) }],
  discovery: { ids: [STEAMAPP_ID, EPICAPP_ID] },
};
function pathPattern(api, game, pattern) { return template(pattern, {}); }
function testLogic(files, gameId) {
  const isPak = files.some((f) => path.extname(f).toLowerCase() === PAK_EXT);
  const isFolder = files.some((f) => path.basename(f) === "LogicMods");
  return Promise.resolve({ supported: gameId === spec.game.id && isPak && isFolder, requiredFiles: [] });
}
function installLogic(files) { return Promise.resolve({ instructions: [{ type: "setmodtype", value: LOGICMODS_ID }] }); }
function applyGame(context, gameSpec) {
  const game = { ...gameSpec.game, queryModPath: () => gameSpec.game.modPath };
  context.registerGame(game);
  (gameSpec.modTypes || []).forEach((type) => {
    context.registerModType(type.id, 60, () => true, (game) => pathPattern(context.api, game, type.targetPath), () => Promise.resolve(false), { name: type.name });
  });
  context.registerModType("stellarblade-uesortablepak", 25, () => true, (game) => pathPattern(context.api, game, path.join("{gamePath}", "SB/Content/Paks/~mods")), () => Promise.resolve(false));
  context.registerInstaller(LOGICMODS_ID, 27, testLogic, installLogic);
}
function main(context) { applyGame(context, spec); return true; }
"#,
        )]);
        let f = &facts[0];
        assert_eq!(f.game_id.as_deref(), Some("stellarblade"));
        assert_eq!(f.executable.as_deref(), Some("SB.exe"));
        assert!(f.stores.steam.contains("3489700"));
        assert!(f.stores.epic.contains("abc123"));
        let logic = f.mod_types.iter().find(|m| m.id == "stellarblade-logicmods").unwrap();
        assert_eq!(logic.target.as_deref(), Some("{gamePath}/SB/Content/Paks/LogicMods"));
        let pak = f.mod_types.iter().find(|m| m.id == "stellarblade-uesortablepak").unwrap();
        assert_eq!(pak.target.as_deref(), Some("{gamePath}/SB/Content/Paks/~mods"));
        let inst = &f.installers[0];
        assert_eq!(inst.priority, Some(27.0));
        assert!(inst.extensions.contains(".pak"));
        assert!(inst.names.contains("LogicMods"));
        assert_eq!(inst.mod_type.as_deref(), Some("stellarblade-logicmods"));
    }

    #[test]
    fn multi_game_registration_inside_loop() {
        let facts = extension(&[(
            "src/index.js",
            r#"
const GAMES = {
  kotor: { id: "kotor", name: "KOTOR", steamId: "32370", exec: "swkotor.exe" },
  kotor2: { id: "kotor2", name: "KOTOR II", steamId: "208580", exec: "swkotor2.exe" },
};
function main(context) {
  Object.keys(GAMES).forEach((key) => {
    const game = GAMES[key];
    context.registerGame({
      id: game.id, name: game.name, queryModPath: () => "override",
      executable: () => game.exec, requiredFiles: [game.exec],
      details: { steamAppId: parseInt(game.steamId, 10) },
    });
  });
  context.registerModType("kotor-root", 10, () => true, () => undefined, () => Promise.resolve(false));
}
"#,
        )]);
        assert_eq!(facts.len(), 2);
        let k2 = facts.iter().find(|f| f.game_id.as_deref() == Some("kotor2")).unwrap();
        assert_eq!(k2.executable.as_deref(), Some("swkotor2.exe"));
        assert!(k2.stores.steam.contains("208580"));
        assert!(!k2.stores.steam.contains("32370"));
        assert!(k2.mod_types.iter().any(|m| m.id == "kotor-root"));
    }

    #[test]
    fn shadowed_names_prefer_the_useful_declaration() {
        let facts = extension(&[(
            "index.js",
            r#"
function other() { let game = undefined; return game; }
function applyGame(context) {
  const game = { id: "abiotic", name: "Abiotic Factor", executable: () => "AbioticFactor.exe", requiredFiles: ["AbioticFactor.exe"], details: { steamAppId: 427410 } };
  context.registerGame(game);
}
"#,
        )]);
        assert_eq!(facts[0].game_id.as_deref(), Some("abiotic"));
        assert_eq!(facts[0].executable.as_deref(), Some("AbioticFactor.exe"));
    }

    #[test]
    fn delegated_tool_family_is_manual() {
        let facts = extension(&[(
            "index.js",
            "/*\nName: X\nStructure: 3rd Party Mod Manager (Fluffy)\n*/\nfunction main(c) { c.registerGame({ id: 're4', executable: () => 're4.exe', requiredFiles: ['re4.exe'], details: { steamAppId: 1 } }); }",
        )]);
        assert_eq!(facts[0].family.as_deref(), Some("3rd Party Mod Manager (Fluffy)"));
        assert_eq!(facts[0].classification.level, Level::Red);
    }
}
