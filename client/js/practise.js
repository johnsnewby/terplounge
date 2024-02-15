
export async function getSources() {
  const response = await fetch("assets/assets.json");
  const list = await response.json();
  let sources = [];
  for (const item of list) {
    let url = "assets/" + item + "/metadata.json";
    let response = await fetch(url);
    let metadata = await response.json();
    await sources.push({ directory: item, metadata: metadata });
  }
  return sources;
}

export const getLanguageName = (lang) => {
  const languageNames = {
    de: "German",
    en: "English",
    fr: "French",
    es: "Spanish",
    it: "Italian"
  }

  return languageNames[lang] || lang;
}

export const getAllLanguages = async () => {
  const sources = await getSources();
  const languages = [
    ...new Set(sources.map((source) => source.metadata.native)),
  ];
  return languages;
}

export const getTargetLanguagesForSourceLanguage = async (sourceLanguage) => {
  const sources = await getSources();
  const metadatas = await sources.map((source) => source.metadata);
  const withSource = metadatas.filter((m) => m.native === sourceLanguage);
  let targetLanguages = [];
  for (var source of withSource) {
    for (var translation of Object.keys(source.translations)) {
      targetLanguages.push(translation);
    }
  }
  return [...new Set(targetLanguages)];
}
    
export async function populateSourceSelector() {
  const sources = await getSources();
  const source_selector = document.getElementById("source-selector");
  for (var i = 0; i < sources.length; i++) {
    const item = sources[i];
    let option = document.createElement("option");
    option.text = item.metadata.name  + " (" + getLanguageName(item.metadata.native) + ")";
    option.value = item.directory;
    source_selector.add(option);
  }
}

export async function getCompatiblePracticeMaterials(source,target) {
  let sources = await getSources();
  let compatiblePracticeMaterials = [];
  for ( var candidate of sources ) {
    if (candidate.metadata.native !== source) {
      continue;
    }
    for (var dest of Object.keys(candidate.metadata.translations)) {
      if (dest === target) {
        compatiblePracticeMaterials.push(candidate);
        continue;
      }
    }
  }
    return compatiblePracticeMaterials;
}

export async function sourceChanged() {
  let source_selector = document.getElementById("source-selector");
  let source_dir = source_selector.value;
  if (source_dir === "") {
    return;
  }
  let asset_dir = "assets/" + source_dir + "/";
  let response = await fetch(asset_dir + "metadata.json");
  let metadata = await response.json();
  let player = document.getElementById("player");
  player.src = asset_dir + metadata.audio;
  let player_figure = document.getElementById("player-figure");
  let player_figure_caption = document.getElementById("player-caption");
  player_figure_caption.innerHTML = metadata.name;
  const langageSelector = document.getElementById("lang");
  while (langageSelector.options.length) {
    langageSelector.remove(0);
  }
  for (const language of metadata.translations) {
    let option = document.createElement("option");
    option.value = Object.keys(language)[0];
    option.text = getLanguageName(option.value);
    langageSelector.add(option);
  }
      
}

