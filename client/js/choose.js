import { getAllLanguages, getLanguageName } from "./js/practise.js";

let allLanguages = await getAllLanguages();
const langageSelector = document.getElementById("sourceLanguage");
while (langageSelector.options.length) {
  langageSelector.remove(0);
}
for (const language of allLanguages) {
  let option = document.createElement("option");
  option.value = language;
  option.text = getLanguageName(option.value);
  langageSelector.add(option);
}

export const sourceUpdated = () => {
  const langageSelector = document.getElementById("sourceLanguage");
  const lang = langageSelector.selected.value;
  if (lang === "") {
    return;
  }
};
