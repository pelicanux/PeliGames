/** Shared entrypoint for title-bar, onboarding and preferences access. */
export function openAddonStore() {
  window.dispatchEvent(new Event("openAddonStore"));
}
