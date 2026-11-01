/* SPDX-License-Identifier: GPL-3.0-or-later
 * Wine answers module queries (GetModuleHandle*, GetProcAddress, GetModuleFileName,
 * EnumProcessModules) under its global loader lock; Windows does not. NeuralHub's
 * module-load hook makes these queries while holding its own lock, so a thread
 * already inside the loader that enters the hook deadlocks against it (seen with
 * Wwise plugin loads at Unreal start-up). For the host's imports only: when the
 * loader lock is free, Wine answers as usual; when another thread holds it, the
 * query is answered read-only from the module list and export tables. */
#include "bridge.h"
#include <winternl.h>
#define PSAPI_VERSION 2
#include <psapi.h>
#include <string.h>
#include <wchar.h>

typedef LDR_DATA_TABLE_ENTRY module_entry;

static PEB *current_peb(void) { return NtCurrentTeb()->Peb; }
static CRITICAL_SECTION *loader_lock(void) { return current_peb()->LoaderLock; }

/* TRUE when this thread now holds the loader lock and may call Wine. */
static BOOL enter_loader(void) { return TryEnterCriticalSection(loader_lock()); }
static void leave_loader(void) { LeaveCriticalSection(loader_lock()); }

static BOOL same_name(const UNICODE_STRING *candidate, const WCHAR *name, size_t length)
{
    size_t count = candidate->Length / sizeof(WCHAR);
    if (count == length) return !_wcsnicmp(candidate->Buffer, name, length);
    /* GetModuleHandle appends ".dll" to a name without an extension. */
    return count == length + 4 && !wcschr(name, '.') && !_wcsnicmp(candidate->Buffer, name, length)
           && !_wcsnicmp(candidate->Buffer + length, L".dll", 4);
}

/* Forward walk only: Wine links a new entry's Flink before publishing it. */
static const module_entry *find_module(const WCHAR *name, const void *address)
{
    PEB_LDR_DATA *ldr = current_peb()->LdrData;
    LIST_ENTRY *head = &ldr->InLoadOrderModuleList;
    size_t length = name ? wcslen(name) : 0;
    BOOL path = name && wcschr(name, '\\');
    for (LIST_ENTRY *link = head->Flink; link != head; link = link->Flink)
    {
        const module_entry *module = (const module_entry *)link;
        if (address && (const BYTE *)address >= (const BYTE *)module->DllBase
            && (const BYTE *)address < (const BYTE *)module->DllBase + module->SizeOfImage) return module;
        if (name && same_name(path ? &module->FullDllName : &module->BaseDllName, name, length)) return module;
    }
    return NULL;
}

static const module_entry *module_of(HMODULE handle)
{
    return find_module(NULL, handle ? (void *)handle : current_peb()->ImageBaseAddress);
}

static BOOL widen(const char *name, WCHAR *out)
{
    return MultiByteToWideChar(CP_ACP, 0, name, -1, out, MAX_PATH) > 0;
}

static HMODULE lookup_handle(const WCHAR *name)
{
    const module_entry *module = find_module(name, NULL);
    if (!module) SetLastError(ERROR_MOD_NOT_FOUND);
    return module ? module->DllBase : NULL;
}

static HMODULE WINAPI module_handle_w(const WCHAR *name)
{
    if (!name || !enter_loader()) return name ? lookup_handle(name) : GetModuleHandleW(NULL);
    HMODULE result = GetModuleHandleW(name);
    leave_loader();
    return result;
}

static HMODULE WINAPI module_handle_a(const char *name)
{
    WCHAR wide[MAX_PATH];
    if (name && widen(name, wide)) return module_handle_w(wide);
    return GetModuleHandleA(name);
}

/* Plain lookups only; reference changes and pinning always go to Wine. */
static BOOL lookup_ex(DWORD flags, const WCHAR *name, HMODULE *module, BOOL *result)
{
    const DWORD address = GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, unchanged = GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
    if (!module || !name || (flags & ~(address | unchanged)) || !(flags & unchanged)) return FALSE;
    const module_entry *found = flags & address ? find_module(NULL, name) : find_module(name, NULL);
    *module = found ? found->DllBase : NULL;
    if (!found) SetLastError(ERROR_MOD_NOT_FOUND);
    *result = found != NULL;
    return TRUE;
}

static BOOL WINAPI module_handle_ex_w(DWORD flags, const WCHAR *name, HMODULE *module)
{
    BOOL result;
    if (enter_loader())
    {
        result = GetModuleHandleExW(flags, name, module);
        leave_loader();
        return result;
    }
    return lookup_ex(flags, name, module, &result) ? result : GetModuleHandleExW(flags, name, module);
}

static BOOL WINAPI module_handle_ex_a(DWORD flags, const char *name, HMODULE *module)
{
    WCHAR wide[MAX_PATH];
    if (flags & GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS) return module_handle_ex_w(flags, (const WCHAR *)name, module);
    if (name && widen(name, wide)) return module_handle_ex_w(flags, wide, module);
    return GetModuleHandleExA(flags, name, module);
}

/* Export lookup; a forwarded export needs the loader and returns NULL here. */
static FARPROC find_export(HMODULE handle, const char *name, BOOL *forwarded)
{
    BYTE *base = (BYTE *)handle;
    IMAGE_NT_HEADERS *nt = (IMAGE_NT_HEADERS *)(base + ((IMAGE_DOS_HEADER *)base)->e_lfanew);
    IMAGE_DATA_DIRECTORY *directory = &nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT];
    if (!directory->VirtualAddress) return NULL;
    IMAGE_EXPORT_DIRECTORY *exports = (IMAGE_EXPORT_DIRECTORY *)(base + directory->VirtualAddress);
    DWORD *functions = (DWORD *)(base + exports->AddressOfFunctions);
    DWORD *names = (DWORD *)(base + exports->AddressOfNames);
    WORD *ordinals = (WORD *)(base + exports->AddressOfNameOrdinals);
    DWORD index = MAXDWORD;
    if (!HIWORD((ULONG_PTR)name)) index = LOWORD((ULONG_PTR)name) - exports->Base;
    else for (DWORD i = 0; i < exports->NumberOfNames; i++)
        if (!strcmp((const char *)(base + names[i]), name)) { index = ordinals[i]; break; }
    if (index >= exports->NumberOfFunctions || !functions[index]) return NULL;
    DWORD rva = functions[index];
    *forwarded = rva >= directory->VirtualAddress && rva < directory->VirtualAddress + directory->Size;
    return *forwarded ? NULL : (FARPROC)(base + rva);
}

static FARPROC WINAPI proc_address(HMODULE module, const char *name)
{
    BOOL forwarded = FALSE;
    if (enter_loader())
    {
        FARPROC result = GetProcAddress(module, name);
        leave_loader();
        return result;
    }
    FARPROC result = module && module_of(module) ? find_export(module, name, &forwarded) : NULL;
    if (forwarded || !module || !module_of(module)) return GetProcAddress(module, name);
    if (!result) SetLastError(ERROR_PROC_NOT_FOUND);
    return result;
}

/* GetModuleFileName semantics: truncate, terminate, report the copied length. */
static DWORD copy_name(const UNICODE_STRING *source, WCHAR *out, DWORD size)
{
    DWORD length = source->Length / sizeof(WCHAR), copied = length < size ? length : size ? size - 1 : 0;
    if (!size) return 0;
    memcpy(out, source->Buffer, copied * sizeof(WCHAR));
    out[copied] = 0;
    if (copied < length) { SetLastError(ERROR_INSUFFICIENT_BUFFER); return size; }
    SetLastError(ERROR_SUCCESS);
    return copied;
}

static DWORD WINAPI module_file_name_w(HMODULE module, WCHAR *out, DWORD size)
{
    if (enter_loader())
    {
        DWORD result = GetModuleFileNameW(module, out, size);
        leave_loader();
        return result;
    }
    const module_entry *found = module_of(module);
    if (!found) { SetLastError(ERROR_MOD_NOT_FOUND); return 0; }
    return copy_name(&found->FullDllName, out, size);
}

static DWORD WINAPI module_file_name_a(HMODULE module, char *out, DWORD size)
{
    WCHAR wide[MAX_PATH];
    if (enter_loader())
    {
        DWORD result = GetModuleFileNameA(module, out, size);
        leave_loader();
        return result;
    }
    if (!module_file_name_w(module, wide, MAX_PATH) || !size) return 0;
    int written = WideCharToMultiByte(CP_ACP, 0, wide, -1, out, size, NULL, NULL);
    if (written > 0) return written - 1;
    out[size - 1] = 0;
    SetLastError(ERROR_INSUFFICIENT_BUFFER);
    return size;
}

static BOOL WINAPI enum_process_modules(HANDLE process, HMODULE *modules, DWORD bytes, DWORD *needed)
{
    if (process != GetCurrentProcess() || enter_loader())
    {
        BOOL result = K32EnumProcessModules(process, modules, bytes, needed);
        if (process == GetCurrentProcess()) leave_loader();
        return result;
    }
    PEB_LDR_DATA *ldr = current_peb()->LdrData;
    LIST_ENTRY *head = &ldr->InLoadOrderModuleList;
    DWORD count = 0;
    for (LIST_ENTRY *link = head->Flink; link != head; link = link->Flink, count++)
        if ((count + 1) * sizeof(HMODULE) <= bytes) modules[count] = ((const module_entry *)link)->DllBase;
    if (needed) *needed = count * sizeof(HMODULE);
    return TRUE;
}

static const struct { const char *name; void *replacement; } replacements[] =
{
    {"GetModuleHandleA", (void *)module_handle_a},
    {"GetModuleHandleW", (void *)module_handle_w},
    {"GetModuleHandleExA", (void *)module_handle_ex_a},
    {"GetModuleHandleExW", (void *)module_handle_ex_w},
    {"GetProcAddress", (void *)proc_address},
    {"GetModuleFileNameA", (void *)module_file_name_a},
    {"GetModuleFileNameW", (void *)module_file_name_w},
    {"K32EnumProcessModules", (void *)enum_process_modules},
};

static BOOL replace_import(void **slot, void *replacement)
{
    DWORD previous;
    if (!VirtualProtect(slot, sizeof(*slot), PAGE_READWRITE, &previous)) return FALSE;
    InterlockedExchangePointer(slot, replacement);
    VirtualProtect(slot, sizeof(*slot), previous, &previous);
    return TRUE;
}

/* Point the module's loader-query imports at the variants above. */
void amdnr_loader_compat(const WCHAR *module_name)
{
    BYTE *base = (BYTE *)GetModuleHandleW(module_name);
    if (!base) { amdnr_log("loader compatibility: %ls is not loaded", module_name); return; }
    IMAGE_NT_HEADERS *nt = (IMAGE_NT_HEADERS *)(base + ((IMAGE_DOS_HEADER *)base)->e_lfanew);
    IMAGE_DATA_DIRECTORY *directory = &nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_IMPORT];
    unsigned replaced = 0;
    for (IMAGE_IMPORT_DESCRIPTOR *import = (IMAGE_IMPORT_DESCRIPTOR *)(base + directory->VirtualAddress);
         directory->VirtualAddress && import->Name; import++)
    {
        if (!import->OriginalFirstThunk) continue;
        IMAGE_THUNK_DATA *names = (IMAGE_THUNK_DATA *)(base + import->OriginalFirstThunk);
        void **slots = (void **)(base + import->FirstThunk);
        for (unsigned i = 0; names[i].u1.AddressOfData; i++)
        {
            if (IMAGE_SNAP_BY_ORDINAL(names[i].u1.Ordinal)) continue;
            const char *function = (const char *)((IMAGE_IMPORT_BY_NAME *)(base + names[i].u1.AddressOfData))->Name;
            for (unsigned r = 0; r < sizeof(replacements) / sizeof(*replacements); r++)
                if (!strcmp(function, replacements[r].name) && replace_import(&slots[i], replacements[r].replacement)) replaced++;
        }
    }
    amdnr_log("loader compatibility: %u loader-query import(s) of %ls no longer wait for the loader lock", replaced, module_name);
}
