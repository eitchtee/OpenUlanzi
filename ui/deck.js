// Deck rendering helpers.
const { invoke } = window.__TAURI__.core;

const pngs = new Map();

/** Blob URL of a key exactly as the deck draws it. Memoised, so repaints are cheap. */
function keyImage(visual, wide = false) {
  const id = JSON.stringify([visual, wide]);
  if (!pngs.has(id)) {
    pngs.set(id, invoke('render', { visual, wide }).then(
      (bytes) => URL.createObjectURL(new Blob([bytes], { type: 'image/png' })),
    ));
  }
  return pngs.get(id);
}

/** Fills `root` with a small read-only copy of what the deck is showing. */
function drawMiniDeck(root, status) {
  const cells = new Map(status.view);
  for (let i = 0; i <= 13; i++) {
    let cell = root.children[i];
    if (!cell) {
      cell = document.createElement('div');
      cell.className = i === 13 ? 'mini-key wide' : 'mini-key';
      root.append(cell);
    }
    const visual = cells.get(i);
    if (!visual) {
      cell.replaceChildren();
      continue;
    }
    const wanted = JSON.stringify(visual);
    if (cell.dataset.visual === wanted) continue;
    cell.dataset.visual = wanted;
    keyImage(visual, i === 13).then((url) => {
      if (cell.dataset.visual !== wanted) return; // a newer state already arrived
      const img = new Image();
      img.alt = '';
      img.src = url;
      // Swap only once it can paint, or the cell shows black for a frame.
      img.decode().catch(() => {}).then(() => {
        if (cell.dataset.visual === wanted) cell.replaceChildren(img);
      });
    });
  }
}
