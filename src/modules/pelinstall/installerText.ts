import {useCallback} from "react";
import {useI18n} from "../../i18n/I18nContext";
import type {Catalog} from "../../i18n/engine";
export function useInstallerText() {
  const {t} = useI18n();
  return useCallback((key: keyof Catalog["pelinstall"]) => t("pelinstall", key), [t]);
}
