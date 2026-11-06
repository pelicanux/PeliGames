import {useMemo} from "react";
import {useI18n} from "./I18nContext";
/** Both launcher and installer use the same registry and persisted preference. */
export function useLanguageChoices() {
  const {languages, preference, language, loading, error, setLanguage, t} = useI18n();
  const options = useMemo(() => [
    {value: "auto", label: t("localization", "automatic")},
    ...languages.filter(item => item.published).map(item => ({value: item.id, label: item.nativeName})),
  ], [languages, t]);
  return {options, preference, language, loading, error, setLanguage, t,
    label: options.find(option => option.value === preference)?.label ?? preference};
}
