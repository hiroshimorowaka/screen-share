let sourcesByType = { window: [], screen: [] };
let activeTab = 'window';

function sourceType(id) {
  return id.startsWith('screen:') ? 'screen' : 'window';
}

function render() {
  const grid = document.getElementById('grid');
  const empty = document.getElementById('empty');
  grid.innerHTML = '';

  const list = sourcesByType[activeTab];
  empty.classList.toggle('hidden', list.length > 0);

  for (const source of list) {
    const el = document.createElement('div');
    el.className = 'source';
    el.tabIndex = 0;

    const icon = source.iconDataUrl
      ? '<img class="app-icon" src="' + source.iconDataUrl + '" alt="">'
      : '<span class="app-icon"></span>';

    el.innerHTML =
      '<img class="thumb" src="' + source.thumbnailDataUrl + '" alt="">' +
      '<div class="label">' + icon + '<span>' + source.name + '</span></div>';

    const choose = () =>
      window.picker.select({
        sourceId: source.id,
        shareAudio: audioCheckbox.checked,
        excludedBinaries: Array.from(excludedBinaries),
      });
    el.addEventListener('click', choose);
    el.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') choose();
    });

    grid.appendChild(el);
  }
}

const audioCheckbox = document.getElementById('audio-checkbox');
const excludePanel = document.getElementById('audio-exclude-panel');
const excludeList = document.getElementById('audio-exclude-list');
const excludeDropdown = document.getElementById('exclude-dropdown');
const excludeDropdownTrigger = document.getElementById('exclude-dropdown-trigger');
const excludeDropdownSummary = document.getElementById('exclude-dropdown-summary');
const excludedBinaries = new Set();
const excludeLabelByBinary = new Map();

function setExcludeDropdownOpen(open) {
  excludeList.classList.toggle('hidden', !open);
  excludeDropdownTrigger.classList.toggle('exclude-dropdown__trigger--open', open);
}

function updateExcludeSummary() {
  const chosen = Array.from(excludedBinaries, (binary) => excludeLabelByBinary.get(binary) ?? binary);
  excludeDropdownSummary.textContent = chosen.length > 0 ? chosen.join(', ') : 'Nenhum';
  excludeDropdownTrigger.classList.toggle('exclude-dropdown__trigger--has-selection', chosen.length > 0);
}

excludeDropdownTrigger.addEventListener('click', () => {
  setExcludeDropdownOpen(excludeList.classList.contains('hidden'));
});

document.addEventListener('click', (event) => {
  if (!excludeDropdown.contains(event.target)) {
    setExcludeDropdownOpen(false);
  }
});

async function refreshExcludePanel() {
  const showPanel = audioCheckbox.checked && activeTab === 'screen';
  excludePanel.classList.toggle('hidden', !showPanel);
  if (!showPanel) {
    setExcludeDropdownOpen(false);
    return;
  }

  const apps = await window.picker.listAudioApps();
  excludeList.innerHTML = '';
  excludeLabelByBinary.clear();
  for (const app of apps) {
    excludeLabelByBinary.set(app.binary, app.label);

    const label = document.createElement('label');
    const input = document.createElement('input');
    input.type = 'checkbox';
    input.className = 'styled-checkbox-input';
    input.checked = excludedBinaries.has(app.binary);
    input.addEventListener('change', () => {
      if (input.checked) {
        excludedBinaries.add(app.binary);
      } else {
        excludedBinaries.delete(app.binary);
      }
      updateExcludeSummary();
    });

    const box = document.createElement('span');
    box.className = 'styled-checkbox-box';
    box.innerHTML =
      '<svg class="styled-checkbox-check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12l5 5L20 6" /></svg>';

    label.appendChild(input);
    label.appendChild(box);
    label.appendChild(document.createTextNode(app.label));
    excludeList.appendChild(label);
  }
  updateExcludeSummary();
}

audioCheckbox.addEventListener('change', () => {
  void refreshExcludePanel();
});

for (const tab of document.querySelectorAll('.tab')) {
  tab.addEventListener('click', () => {
    activeTab = tab.dataset.tab;
    for (const t of document.querySelectorAll('.tab')) {
      t.classList.toggle('active', t === tab);
    }
    render();
    void refreshExcludePanel();
  });
}

window.picker.onSources((sources) => {
  sourcesByType = { window: [], screen: [] };
  for (const source of sources) {
    sourcesByType[sourceType(source.id)].push(source);
  }
  render();
});
