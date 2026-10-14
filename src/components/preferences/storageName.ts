export const storageNameInput = (value: string) => value.replace(/[^A-Za-z0-9_.-]/g, "").replace(/^[^A-Za-z0-9]+/, "").slice(0, 16);
