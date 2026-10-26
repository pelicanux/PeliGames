// Model GE's direct function-address relocation and its dlsym scopes.
#include <assert.h>
#include <dlfcn.h>

int probe_bridge_input(void *handle) {
    int (*original)(void *, void *, void *, void *) = dlsym(handle, "XCheckIfEvent");
    assert(original);
    return original((void *)11, (void *)22, (void *)33, (void *)44);
}

void *probe_bridge_default(void) {
    return dlsym(RTLD_DEFAULT, "XCheckIfEvent");
}

void *probe_bridge_local(void *handle) {
    return dlsym(handle, "XCheckIfEvent");
}

extern int XCheckIfEvent(void *, void *, void *, void *);
void *probe_bridge_bound(void) {
    return (void *)&XCheckIfEvent;
}
