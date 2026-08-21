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

    const choose = () => window.picker.select(source.id);
    el.addEventListener('click', choose);
    el.addEventListener('keydown', (event) => {
      if (event.key === 'Enter' || event.key === ' ') choose();
    });

    grid.appendChild(el);
  }
}

for (const tab of document.querySelectorAll('.tab')) {
  tab.addEventListener('click', () => {
    activeTab = tab.dataset.tab;
    for (const t of document.querySelectorAll('.tab')) {
      t.classList.toggle('active', t === tab);
    }
    render();
  });
}

window.picker.onSources((sources) => {
  sourcesByType = { window: [], screen: [] };
  for (const source of sources) {
    sourcesByType[sourceType(source.id)].push(source);
  }
  render();
});
