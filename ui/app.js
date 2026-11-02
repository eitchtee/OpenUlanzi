const KEYS = [...Array(13).keys()].map(String);
const KNOBS = ['knob1', 'knob2', 'knob3'];
const SCREENS = [...KEYS, 'wide'];
const CONTROL_NAMES = { wide: 'Wide screen', side1: 'Left side button', side2: 'Right side button', knob1: 'Left knob', knob2: 'Middle knob', knob3: 'Right knob' };
const APP_LAYER = '@app';
const DEFAULT_PROFILE = '@default';
const BACK_KEY = '0';
const BACK_BINDING = { label: 'Back', icon: 'ph:arrow-left-bold' };
const WIDE_DISPLAYS = [
  ['image', 'Label and icon'], ['page_indicator', 'Page number'], ['dial', 'Analog clock'], ['time', 'Time'],
  ['time_weekday', 'Time and weekday'], ['time_date', 'Time and date'], ['date_time_weekday', 'Date, time and weekday'],
  ['stats', 'CPU and memory'],
];

/* Every entry can be dragged onto a control, or clicked to apply it to the selected one. */
const ACTION_GROUPS = [
  ['Pages', [
    { name: 'Open page', icon: 'folder-open-bold', apply: (b, ctx) => { b.action = { page: ctx.otherPage() }; } },
    { name: 'Open folder', icon: 'folder-bold', folder: true, apply: (b, ctx) => { b.action = { page: ctx.folder() }; b.icon ||= 'ph:folder-bold'; } },
    { name: 'Go back', icon: 'arrow-u-up-left-bold', apply: (b) => { b.action = 'back'; b.label ||= 'Back'; b.icon ||= 'ph:arrow-u-up-left-bold'; } },
    { name: 'Next page', icon: 'caret-right-bold', apply: (b) => { b.action = 'next_page'; b.icon ||= 'ph:caret-right-bold'; } },
    { name: 'Previous page', icon: 'caret-left-bold', apply: (b) => { b.action = 'prev_page'; b.icon ||= 'ph:caret-left-bold'; } },
    { name: 'Switch profile', icon: 'user-switch-bold', apply: (b) => { b.action = { profile: state.cfg.profile[0].name }; } },
    { name: 'Go to focused app profile', icon: 'app-window-bold', apply: (b) => { b.action = { profile: APP_LAYER }; b.label ||= 'App'; } },
    { name: 'Go to default profile', icon: 'house-bold', apply: (b) => { b.action = { profile: DEFAULT_PROFILE }; b.label ||= 'Home'; b.icon ||= 'ph:house-bold'; } },
    { name: 'Page number', icon: 'hash-bold', screen: true, apply: (b) => { b.display = 'page_indicator'; } },
  ]],
  ['Hold to show', [
    { name: 'Hold for a page', icon: 'hand-grabbing-bold', hold: true, apply: (b, ctx) => { b.hold = { show: ctx.otherPage() }; } },
    { name: 'Hold for the focused app', icon: 'app-window-bold', hold: true, apply: (b) => { b.hold = { show: APP_LAYER }; b.label ||= 'App'; } },
    { name: 'Hold to run an action', icon: 'timer-bold', hold: true, apply: (b) => { b.hold = { hotkey: '' }; } },
  ]],
  ['System', [
    { name: 'Hotkey', icon: 'keyboard-bold', apply: (b) => { b.action = { hotkey: '' }; } },
    { name: 'Type text', icon: 'text-t-bold', apply: (b) => { b.action = { text: '' }; } },
    { name: 'Open app or file', icon: 'rocket-launch-bold', apply: (b) => { b.action = { open: '' }; } },
    { name: 'Close app', icon: 'x-square-bold', apply: (b) => { b.action = { close: '' }; } },
    { name: 'Website', icon: 'globe-bold', apply: (b) => { b.action = { open: 'https://' }; b.icon ||= 'ph:globe-bold'; } },
    { name: 'Scroll up', icon: 'arrow-line-up-bold', apply: (b) => { b.action = { scroll_up: 3 }; } },
    { name: 'Scroll down', icon: 'arrow-line-down-bold', apply: (b) => { b.action = { scroll_down: 3 }; } },
    { name: 'Scroll left', icon: 'arrow-line-left-bold', apply: (b) => { b.action = { scroll_left: 3 }; } },
    { name: 'Scroll right', icon: 'arrow-line-right-bold', apply: (b) => { b.action = { scroll_right: 3 }; } },
    { name: 'Deck brightness', icon: 'sun-bold', apply: (b) => { b.action = { brightness: 10 }; } },
    { name: 'Block input', icon: 'lock-bold', apply: (b) => { b.action = 'block_input'; b.icon ||= 'ph:lock-bold'; } },
    { name: 'Unblock input', icon: 'lock-open-bold', apply: (b) => { b.action = 'unblock_input'; } },
  ]],
  ['Media', [
    { name: 'Play or pause', icon: 'play-pause-bold', apply: (b) => media(b, 'play_pause', 'Play', 'play-pause-fill') },
    { name: 'Next track', icon: 'skip-forward-bold', apply: (b) => media(b, 'next_track', 'Next', 'skip-forward-fill') },
    { name: 'Previous track', icon: 'skip-back-bold', apply: (b) => media(b, 'prev_track', 'Previous', 'skip-back-fill') },
    { name: 'Mute', icon: 'speaker-x-bold', apply: (b) => media(b, 'mute', 'Mute', 'speaker-x-fill') },
    { name: 'Volume up', icon: 'speaker-high-bold', apply: (b) => media(b, 'volume_up', 'Louder', 'speaker-high-fill') },
    { name: 'Volume down', icon: 'speaker-low-bold', apply: (b) => media(b, 'volume_down', 'Quieter', 'speaker-low-fill') },
  ]],
  ['Focused app volume', [
    { name: 'App volume up', icon: 'speaker-simple-high-bold', apply: (b) => { b.action = { app_volume: 5 }; } },
    { name: 'App volume down', icon: 'speaker-simple-low-bold', apply: (b) => { b.action = { app_volume: -5 }; } },
    { name: 'Reset app volume', icon: 'arrow-counter-clockwise-bold', apply: (b) => { b.action = 'app_volume_reset'; } },
    { name: 'App volume knob', icon: 'speaker-simple-high-bold', knob: true, apply: (b) => { b.left = { app_volume: -5 }; b.right = { app_volume: 5 }; b.action = 'app_volume_reset'; b.label ||= 'App'; } },
  ]],
  ['Knobs', [
    { name: 'Volume knob', icon: 'speaker-high-bold', knob: true, apply: (b) => { b.left = { hotkey: 'volume_down' }; b.right = { hotkey: 'volume_up' }; b.action = { hotkey: 'mute' }; b.label ||= 'Volume'; } },
    { name: 'Vertical scroll knob', icon: 'arrows-down-up-bold', knob: true, apply: (b) => { b.left = { scroll_up: 3 }; b.right = { scroll_down: 3 }; b.label ||= 'Scroll'; } },
    { name: 'Horizontal scroll knob', icon: 'arrows-left-right-bold', knob: true, apply: (b) => { b.left = { scroll_left: 3 }; b.right = { scroll_right: 3 }; b.label ||= 'Scroll'; } },
    { name: 'Brightness knob', icon: 'sun-bold', knob: true, apply: (b) => { b.left = { brightness: -10 }; b.right = { brightness: 10 }; b.label ||= 'Brightness'; } },
  ]],
  ['Key look', [
    { name: 'Change look', icon: 'palette-bold', apply: (b) => { b.action = { set_look: {} }; } },
    { name: 'Reset look', icon: 'arrow-counter-clockwise-bold', apply: (b) => { b.action = { reset_look: {} }; } },
  ]],
  ['Several actions', [
    { name: 'Sequence', icon: 'stack-bold', apply: (b) => { b.action = { sequence: [] }; } },
    { name: 'Toggle between actions', icon: 'swap-bold', apply: (b) => { b.action = { toggle: [] }; } },
    { name: 'Random action', icon: 'shuffle-bold', apply: (b) => { b.action = { random: [] }; } },
    { name: 'Wait', icon: 'hourglass-bold', wait: true, apply: (b) => { b.action = { delay: 200 }; } },
  ]],
  ['Sound', [
    { name: 'Play sound file', icon: 'music-note-bold', apply: (b) => { b.action = { play_sound: '' }; } },
    { name: 'Stop sound', icon: 'stop-bold', apply: (b) => { b.action = 'stop_sound'; } },
  ]],
  // Offered only while OBS is connected, because that is when the names can be picked.
  ['OBS Studio', [
    { name: 'Record', icon: 'record-bold', obs: true, apply: (b) => obsAction(b, 'toggle_record', 'Record', 'record-fill') },
    { name: 'Pause recording', icon: 'pause-bold', obs: true, apply: (b) => obsAction(b, 'toggle_record_pause', 'Pause', 'pause-fill') },
    { name: 'Stream', icon: 'broadcast-bold', obs: true, apply: (b) => obsAction(b, 'toggle_stream', 'Stream', 'broadcast-fill') },
    { name: 'Virtual camera', icon: 'webcam-bold', obs: true, apply: (b) => obsAction(b, 'toggle_virtual_cam', 'Virtual cam', 'webcam-fill') },
    { name: 'Replay buffer', icon: 'repeat-bold', obs: true, apply: (b) => obsAction(b, 'toggle_replay_buffer', 'Replay', 'repeat-fill') },
    { name: 'Save replay', icon: 'floppy-disk-bold', obs: true, apply: (b) => obsAction(b, 'save_replay', 'Save replay', 'floppy-disk-fill') },
    { name: 'Studio mode', icon: 'monitor-play-bold', obs: true, apply: (b) => obsAction(b, 'toggle_studio_mode', 'Studio', 'monitor-play-fill') },
    { name: 'Transition', icon: 'arrows-left-right-bold', obs: true, apply: (b) => obsAction(b, 'transition', 'Transition', 'arrows-left-right-fill') },
    { name: 'Scene transition effect', icon: 'swap-bold', obs: true, apply: (b) => obsAction(b, { scene_transition: obsFirst('transitions') }, 'Effect', 'swap-fill') },
    { name: 'Screenshot', icon: 'camera-bold', obs: true, apply: (b) => obsAction(b, 'screenshot', 'Screenshot', 'camera-fill') },
    { name: 'Scene collection', icon: 'stack-bold', obs: true, apply: (b) => obsAction(b, { scene_collection: obsFirst('collections') }, 'Collection', 'stack-fill') },
    { name: 'Profile', icon: 'user-circle-bold', obs: true, apply: (b) => obsAction(b, { profile: obsFirst('profiles') }, 'Profile', 'user-circle-fill') },
    { name: 'Scene', icon: 'monitor-bold', obs: true, apply: (b) => obsAction(b, { scene: obsFirst('scenes') }, 'Scene', 'monitor-fill') },
    { name: 'Source visibility', icon: 'eye-bold', obs: true, apply: (b) => obsAction(b, { toggle_source: { scene: obsFirst('scenes'), source: '' } }, 'Source', 'eye-fill') },
    { name: 'Chapter marker', icon: 'bookmark-simple-bold', obs: true, apply: (b) => obsAction(b, { chapter: '' }, 'Chapter', 'bookmark-simple-fill') },
    { name: 'Filter', icon: 'funnel-bold', obs: true, apply: (b) => obsAction(b, { toggle_filter: { source: obsFirst('scenes'), filter: '' } }, 'Filter', 'funnel-fill') },
    { name: 'Media source control', icon: 'play-circle-bold', obs: true, apply: (b) => obsAction(b, { media: { input: obsFirst('inputs'), action: 'play' } }, 'Media', 'play-circle-fill') },
    { name: 'Mute input', icon: 'microphone-slash-bold', obs: true, apply: (b) => obsAction(b, { toggle_mute: obsFirst('inputs') }, 'Mute', 'microphone-slash-fill') },
    { name: 'Input volume', icon: 'sliders-horizontal-bold', obs: true, apply: (b) => obsAction(b, { volume: { input: obsFirst('inputs'), db: 2 } }, 'Volume', 'sliders-horizontal-fill') },
    { name: 'Mixer knob', icon: 'sliders-horizontal-bold', obs: true, knob: true, apply: (b) => {
      const input = obsFirst('inputs');
      b.left = { obs: { volume: { input, db: -2 } } };
      b.right = { obs: { volume: { input, db: 2 } } };
      b.action = { obs: { toggle_mute: input } };
      b.label ||= 'Mixer';
    } },
  ]],
];
const BUILT_IN_GROUPS = ACTION_GROUPS.length;
let PALETTE = ACTION_GROUPS.flatMap(([, items]) => items);

/** Plugin groups follow the built-in ones, so palette positions of those never move. */
function usePlugins(plugins) {
  state.plugins = plugins;
  ACTION_GROUPS.length = BUILT_IN_GROUPS;
  for (const plugin of plugins.filter((p) => p.actions.length)) {
    ACTION_GROUPS.push([plugin.name, plugin.actions.map((action) => ({
      name: action.name,
      icon: action.icon.replace(/^ph:/, '') || 'puzzle-piece-bold',
      apply: (b) => {
        const params = Object.fromEntries(action.fields.map((f) => [f.key, f.default]));
        b.action = { plugin: { id: plugin.id, action: action.id, params } };
        b.label ||= action.name;
        if (action.icon) b.icon ||= action.icon;
      },
    }))]);
    for (const action of plugin.actions) ACTION_NAMES[`plugin:${plugin.id}:${action.id}`] = `${plugin.name}: ${action.name}`;
  }
  PALETTE = ACTION_GROUPS.flatMap(([, items]) => items);
}

function obsAction(b, obs, label, glyph) {
  b.action = { obs };
  b.label ||= label;
  b.icon ||= `ph:${glyph}`;
}

const obsFirst = (list) => state.status?.obs?.[list]?.[0] ?? '';

function media(b, hotkey, label, icon) {
  b.action = { hotkey };
  b.label ||= label;
  b.icon ||= `ph:${icon}`;
}

const state = {
  device: '', devices: [], plugins: [], cfg: null, profile: 0, page: 0,
  /** Every selected control. Shift+click ranges start at the anchor. */
  selection: new Set(), anchor: null,
  /** The one selected control, or null when none or several are. */
  get selected() { return this.selection.size === 1 ? [...this.selection][0] : null; },
  set selected(control) { this.selection = new Set(control === null ? [] : [control]); this.anchor = control; },
  view: 'keys', status: null, icons: new Map(), collapsed: new Set(), clipboard: null, armed: null };
const $ = (id) => document.getElementById(id);

function remember(key, value) {
  try { localStorage.setItem(key, value); } catch { /* preferences are optional */ }
}
function recall(key) {
  try { return localStorage.getItem(key); } catch { return null; }
}

/** Tiny DOM builder: h('div', { class: 'x', onclick: f }, child, 'text'). */
function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k.startsWith('on')) el.addEventListener(k.slice(2), v);
    else if (k in el && k !== 'list') el[k] = v;
    else el.setAttribute(k, v);
  }
  el.append(...children.filter((c) => c !== null && c !== undefined));
  return el;
}

/** Inline SVG from the embedded set. The markup comes from our own binary, not from user data. */
function icon(name) {
  const span = h('span', { class: 'glyph', 'aria-hidden': 'true' });
  const inner = state.icons.get(name);
  if (inner) span.innerHTML = `<svg viewBox="0 0 256 256" fill="currentColor">${inner}</svg>`;
  return span;
}

/** A titled block; consecutive ones are split by a line. */
const section = (title, ...children) => h('section', { class: 'form-section' }, h('h3', {}, title), ...children);
const field = (label, control, hint) => h('label', { class: 'field' }, h('span', {}, label), control, hint ? h('p', { class: 'hint' }, hint) : null);
const profile = () => state.cfg.profile[state.profile];
const page = () => profile().page[state.page];
const isBackKey = (control) => !!page().folder && control === BACK_KEY;
const binding = (control) => page().keys[control];
const controlName = (c) => CONTROL_NAMES[c] ?? `Key ${Number(c) + 1}`;
const isKnob = (c) => KNOBS.includes(c);
const hasScreen = (c) => SCREENS.includes(c);
const hasDials = () => state.cfg.model === 'd200x';
/** Controls in the order a Shift+click range runs through them. */
const ORDER = [...KEYS, 'wide', 'side1', 'side2', ...KNOBS];
const selectedList = () => ORDER.filter((c) => state.selection.has(c));
/** Settings carry whole only between controls of the same kind. */
const kindOf = (c) => (isKnob(c) ? 'knob' : c === 'wide' ? 'wide' : hasScreen(c) ? 'key' : 'side');
const ACTION_FIELDS = ['action', 'double', 'hold', 'hold_ms', 'left', 'right', 'press_left', 'press_right'];

function notify(text, bad = false) {
  const el = $('notice');
  el.textContent = text;
  el.className = bad ? 'notice bad' : 'notice';
  el.hidden = false;
  clearTimeout(notify.timer);
  if (!bad) notify.timer = setTimeout(() => { el.hidden = true; }, 2600);
}

let saveTimer = null;

async function flushSave() {
  if (saveTimer === null) return;
  clearTimeout(saveTimer);
  saveTimer = null;
  try {
    await invoke('save_config', { device: state.device, config: state.cfg });
    $('notice').hidden = true;
  } catch (e) {
    notify(`Not saved: ${e}`, true);
  }
}

function save() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(flushSave, 250);
}

/** Change a control's binding, creating it if needed and dropping it if nothing is left. */
function edit(control, change, persist = true) {
  const b = page().keys[control] ?? {};
  change(b);
  for (const k of Object.keys(b)) if (b[k] === '' || b[k] === null || b[k] === undefined) delete b[k];
  if (Object.keys(b).length) page().keys[control] = b;
  else delete page().keys[control];
  if (persist) save();
  drawDeck();
}

const context = {
  otherPage() {
    const other = profile().page.find((p, i) => i !== state.page);
    return other ? other.name : page().name;
  },
  folder() {
    return profile().page.find((p) => p.folder)?.name ?? page().name;
  },
};

function wideDisplay(b) {
  if (b?.display) return b.display;
  return b?.label || b?.icon || b?.image || b?.color || b?.background ? 'image' : state.cfg.wide;
}

function visualOf(b, control) {
  const display = control === 'wide' ? wideDisplay(b) : (b?.display ?? 'image');
  return {
    label: b?.label ?? '',
    icon: b?.icon ?? '',
    color: b?.color ?? state.cfg.color,
    background: b?.background ?? state.cfg.background,
    image: b?.image ?? '',
    image_crop: b?.image_crop ?? null,
    icon_crop: b?.icon_crop ?? null,
    display,
    big: display === 'page_indicator' ? '1/2' : '',
  };
}

// ---------------------------------------------------------------- keys view
function focusControl(control) {
  document.querySelector(`[data-control="${control}"]`)?.focus();
}

function select(control) {
  state.selected = control;
  drawDeck();
  drawInspector();
  if (control !== null) focusControl(control);
}

/** Click picks one control; Ctrl+click adds or removes one, Shift+click a range. */
function pickControl(e, control) {
  if (e.ctrlKey || e.metaKey) {
    const next = new Set(state.selection);
    if (next.has(control)) next.delete(control);
    else next.add(control);
    state.selection = next;
    state.anchor = control;
  } else if (e.shiftKey && state.anchor !== null) {
    const [from, to] = [ORDER.indexOf(state.anchor), ORDER.indexOf(control)].sort((a, b) => a - b);
    const shown = (c) => !isBackKey(c) && document.querySelector(`.deck [data-control="${c}"]:not(.locked)`);
    state.selection = new Set(ORDER.slice(from, to + 1).filter(shown));
  } else {
    return select(control);
  }
  drawDeck();
  drawInspector();
  focusControl(control);
}

function clearControls(controls) {
  for (const c of controls) delete page().keys[c];
  save();
  drawDeck();
  drawInspector();
  focusControl(controls[0]);
}

function copyControls(controls) {
  const items = controls.filter((c) => binding(c) && !isBackKey(c)).map((control) => ({ control, binding: structuredClone(binding(control)) }));
  if (!items.length) return notify('Nothing to copy: the selection is empty.');
  state.clipboard = items;
  notify(items.length === 1 ? `Copied ${controlName(items[0].control)}.` : `Copied ${items.length} controls.`);
}

/** Where a copied key lands when several are pasted onto one target: same layout, shifted. */
function shifted(items, target, control) {
  const first = items.find((it) => KEYS.includes(it.control))?.control;
  if (!KEYS.includes(control)) return control; // the wide screen, side buttons and knobs go back where they were
  if (first === undefined || !KEYS.includes(target)) return control;
  const row = Math.floor(control / 5) + Math.floor(target / 5) - Math.floor(first / 5);
  const col = (control % 5) + (target % 5) - (first % 5);
  const index = row * 5 + col;
  return row >= 0 && col >= 0 && col < 5 && index < KEYS.length ? String(index) : null;
}

function pasteControls(targets) {
  const items = state.clipboard;
  if (!items) return;
  let pairs;
  if (items.length === 1) pairs = targets.map((t) => [items[0], t]);
  else if (targets.length === 1) pairs = items.map((it) => [it, shifted(items, targets[0], it.control)]);
  else if (targets.length === items.length) pairs = items.map((it, i) => [it, targets[i]]);
  else return notify(`${items.length} controls were copied and ${targets.length} are selected. Select one, or as many as were copied.`, true);
  let skipped = 0;
  let partial = false;
  const pasted = [];
  for (const [item, target] of pairs) {
    const [from, to] = [kindOf(item.control), target && kindOf(target)];
    if (target === null || isBackKey(target) || (from === 'knob') !== (to === 'knob')) {
      skipped++;
      continue;
    }
    if (from === to) {
      page().keys[target] = structuredClone(item.binding);
    } else {
      // The wide screen, keys and side buttons draw differently, so only what they do carries over.
      const next = Object.fromEntries(Object.entries(binding(target) ?? {}).filter(([k]) => !ACTION_FIELDS.includes(k)));
      for (const k of ACTION_FIELDS) if (item.binding[k] !== undefined) next[k] = structuredClone(item.binding[k]);
      if (from !== 'wide' && to !== 'wide' && item.binding.label) next.label = item.binding.label;
      if (Object.keys(next).length) page().keys[target] = next;
      else delete page().keys[target];
      partial = true;
    }
    pasted.push(target);
  }
  if (pasted.length) {
    state.selection = new Set(pasted);
    save();
    drawDeck();
    drawInspector();
  }
  const notes = [];
  if (partial) notes.push('Only the actions carried over between the wide screen, keys and side buttons.');
  if (skipped) notes.push(`${skipped} skipped: knob settings only go on knobs, nothing goes on a folder's Back key, and keys that would land off the deck are dropped.`);
  if (notes.length) notify(notes.join(' '), !pasted.length);
}

function closeMenu(restoreFocus = false) {
  const menu = document.getElementById('menu');
  if (!menu) return;
  menu.remove();
  if (restoreFocus) state.menuReturn?.focus?.();
}

/** Where a menu opens: the pointer, or the element itself when opened from the keyboard. */
function menuPoint(event) {
  if (event.clientX || event.clientY) return { x: event.clientX, y: event.clientY };
  const box = event.currentTarget.getBoundingClientRect();
  return { x: box.left, y: box.bottom };
}

function openMenu(at, items) {
  closeMenu();
  state.menuReturn = document.activeElement;
  const entries = items.filter(Boolean).map((item) => (item === '-' ? h('hr') : h('button', {
    role: 'menuitem',
    class: item.danger ? 'danger' : '',
    disabled: !!item.disabled,
    onclick: () => { closeMenu(); item.run(); },
  }, item.icon ? icon(item.icon) : null, item.label, item.checked ? h('span', { class: 'tick' }, icon('check-bold')) : null)));
  const menu = h('div', { id: 'menu', class: 'menu', role: 'menu' }, ...entries);
  menu.addEventListener('keydown', (e) => {
    const buttons = [...menu.querySelectorAll('button:not(:disabled)')];
    const step = { ArrowDown: 1, ArrowUp: -1 }[e.key];
    if (!step || !buttons.length) return;
    e.preventDefault();
    const at = buttons.indexOf(document.activeElement);
    buttons[(at + step + buttons.length) % buttons.length].focus();
  });
  document.body.append(menu);
  // Flip at the window edges, and grow from whichever corner sits at the pointer.
  const flipX = at.x + menu.offsetWidth + 4 > innerWidth;
  const flipY = at.y + menu.offsetHeight + 4 > innerHeight;
  menu.style.left = `${flipX ? at.x - menu.offsetWidth : at.x}px`;
  menu.style.top = `${flipY ? Math.max(4, at.y - menu.offsetHeight) : at.y}px`;
  menu.style.transformOrigin = `${flipX ? 'right' : 'left'} ${flipY ? 'bottom' : 'top'}`;
  menu.querySelector('button:not(:disabled)')?.focus({ preventScroll: true });
}

function controlMenu(at, control) {
  const targets = state.selection.has(control) ? selectedList() : [control];
  const used = targets.some((c) => binding(c));
  openMenu(at, [
    { label: 'Copy', icon: 'copy-bold', disabled: !used, run: () => copyControls(targets) },
    { label: 'Paste', icon: 'clipboard-text-bold', disabled: !state.clipboard, run: () => pasteControls(targets) },
    '-',
    { label: 'Clear', icon: 'eraser-bold', danger: true, disabled: !used, run: () => clearControls(targets) },
  ]);
}

function iconButton(glyph, label, run) {
  return h('button', { type: 'button', class: 'icon-button', title: label, 'aria-label': label, onclick: run }, icon(glyph));
}

function confirmDialog(title, text, action, run) {
  openDialog(dialogHead(title), h('p', {}, text), dialogFoot(
    h('button', { class: 'plain', onclick: () => $('dialog').close() }, 'Cancel'),
    h('button', { class: 'plain danger', onclick: () => { $('dialog').close(); run(); } }, action)));
  $('dialog').classList.add('narrow');
}

function wireDrop(el, control) {
  el.addEventListener('dragover', (e) => { e.preventDefault(); el.classList.add('over'); });
  el.addEventListener('dragleave', () => el.classList.remove('over'));
  el.addEventListener('drop', (e) => {
    e.preventDefault();
    el.classList.remove('over');
    drop(e.dataTransfer.getData('text/plain'), control);
  });
}

function wireDrag(el, control) {
  el.draggable = true;
  el.dataset.control = control;
  el.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    e.stopPropagation();
    controlMenu(menuPoint(e), control);
  });
  el.addEventListener('keydown', (e) => {
    if (e.key === 'Delete') {
      e.preventDefault();
      clearControls(state.selection.has(control) ? selectedList() : [control]);
    }
  });
  el.addEventListener('dragstart', (e) => {
    e.dataTransfer.setData('text/plain', `key:${control}`);
    el.classList.add('dragging');
  });
  el.addEventListener('dragend', () => el.classList.remove('dragging'));
  wireDrop(el, control);
}

function drop(data, target) {
  const [kind, value] = data.split(':');
  if (kind === 'palette') return applyPalette(PALETTE[Number(value)], target);
  if (kind !== 'key' || value === target || isBackKey(target)) return;
  if (isKnob(value) !== isKnob(target)) return notify('Knobs can only swap with knobs.', true);
  const keys = page().keys;
  const [from, to] = [keys[value], keys[target]];
  delete keys[value];
  delete keys[target];
  if (from) keys[target] = from;
  if (to) keys[value] = to;
  state.selected = target;
  save();
  draw();
}

/** Folder actions need a folder to point at, so the first one creates it. */
function ensureFolder(item) {
  if (item.folder && !profile().page.some((p) => p.folder)) {
    profile().page.push(newFolder());
    drawProfiles();
  }
}

function applyPalette(item, target) {
  ensureFolder(item);
  if (state.armed) return state.armed(item);
  if (item.wait) return notify('Wait only works inside a sequence.', true);
  if (target !== null && isBackKey(target)) return notify('A folder always keeps Back on this key.', true);
  if (target === null) return notify('Select a control first, or drag this onto one.');
  if (item.hold && isKnob(target)) return notify('Knobs cannot be held, because they only report a click.', true);
  if (item.knob && !isKnob(target)) return notify(`${item.name} goes on a knob.`, true);
  if (item.screen && !hasScreen(target)) return notify(`${item.name} needs a screen.`, true);
  edit(target, (b) => item.apply(b, context));
  select(target);
}

/* What a control does, as marks: their shape names the state, and each keeps its place. */
const KEY_STATES = [
  ['tap', 'Click', (b) => b.action],
  ['double', 'Double click', (b) => b.double],
  ['long', 'Hold', (b) => b.hold],
];
const KNOB_STATES = [
  ['tap', 'Click', (b) => b.action],
  ['double', 'Double click', (b) => b.double],
  ['left', 'Turn left', (b) => b.left],
  ['right', 'Turn right', (b) => b.right],
  ['press-left', 'Turn left while pressed', (b) => b.press_left],
  ['press-right', 'Turn right while pressed', (b) => b.press_right],
];

const MARK_GLYPHS = {
  left: 'arrow-arc-left-bold',
  right: 'arrow-arc-right-bold',
  'press-left': 'arrow-arc-left-bold',
  'press-right': 'arrow-arc-right-bold',
};

function mark(kind, on) {
  const glyph = MARK_GLYPHS[kind];
  return h('span', { class: `mark ${kind}${on ? ' on' : ''}` }, glyph ? icon(glyph) : null);
}

function addMarks(el, b, states) {
  const active = states.filter(([, , has]) => b && has(b));
  el.append(h('span', { class: 'marks', 'aria-hidden': 'true' },
    ...states.map((state) => mark(state[0], active.includes(state)))));
  if (active.length) el.title = active.map(([, name]) => name).join(', ');
}

function keyButton(control, shown) {
  const locked = isBackKey(control);
  const b = locked ? BACK_BINDING : binding(control);
  const classes = ['ctl', 'key'];
  if (control === 'wide') classes.push('wide');
  const display = visualOf(b, control).display;
  const firmware = !['image', 'page_indicator'].includes(display);
  if (firmware) classes.push('firmware');
  const el = h('button', {
    class: classes.join(' '),
    'aria-pressed': String(state.selection.has(control)),
    'aria-label': `${controlName(control)}${b?.label ? `: ${b.label}` : ''}`,
    onclick: (e) => (locked ? notify('A folder always keeps Back on this key.') : pickControl(e, control)),
  });
  if (firmware) el.dataset.face = WIDE_DISPLAYS.find(([id]) => id === display)?.[1] ?? '';
  // A firmware face is drawn over the background image alone, as the deck does.
  const look = firmware ? (b?.image ? { ...visualOf(b, control), label: '', icon: '', display: 'image', big: '' } : null) : visualOf(b, control);
  if (look) {
    // Keep showing the old image until the new one can paint, or the key flashes black.
    if (shown) el.append(shown);
    const visual = JSON.stringify(look);
    el.dataset.visual = visual;
    keyImage(look, control === 'wide').then(async (url) => {
      const img = h('img', { src: url, alt: '', draggable: false });
      await img.decode().catch(() => {});
      if (el.dataset.visual !== visual) return;
      const old = el.querySelector('img');
      if (old) old.replaceWith(img);
      else el.prepend(img);
    });
  }
  if (locked) {
    el.classList.add('locked');
    el.dataset.control = control;
  } else {
    addMarks(el, b, KEY_STATES);
    wireDrag(el, control);
  }
  return el;
}

function sideButton(control) {
  const b = binding(control);
  const el = h('button', {
    class: 'ctl side',
    'aria-pressed': String(state.selection.has(control)),
    onclick: (e) => pickControl(e, control),
  }, b?.label || (b ? 'In use' : 'Empty'));
  addMarks(el, b, KEY_STATES);
  wireDrag(el, control);
  return el;
}

function knob(control) {
  const b = binding(control);
  const el = h('button', {
    class: 'ctl knob',
    'aria-pressed': String(state.selection.has(control)),
    'aria-label': `${controlName(control)}${b?.label ? `: ${b.label}` : ''}`,
    onclick: (e) => pickControl(e, control),
  });
  addMarks(el, b, KNOB_STATES);
  wireDrag(el, control);
  return h('div', { class: 'knob-slot' }, el, h('span', { class: 'knob-label' }, b?.label || (b ? 'In use' : 'Empty')));
}

function drawDeck() {
  const deck = document.querySelector('.deck');
  if (!deck) return;
  const shown = new Map();
  for (const el of deck.querySelectorAll('.key[data-control]')) {
    const img = el.querySelector('img');
    if (img) shown.set(el.dataset.control, img);
  }
  // Only the D200X has knobs and side buttons, and USB can't tell, so the Settings model decides.
  deck.classList.toggle('folder', !!page().folder);
  const extras = hasDials() ? [sideButton('side1'), sideButton('side2'), ...KNOBS.map(knob)] : [];
  deck.replaceChildren(
    ...KEYS.map((c) => keyButton(c, shown.get(c))), keyButton('wide', shown.get('wide')), ...extras,
  );
}

function actionLists(action, visit) {
  if (!action || typeof action === 'string') return;
  visit(action);
  actionLists(action.one_shot, visit);
  for (const list of [action.sequence, action.toggle, action.random]) list?.forEach((a) => actionLists(a, visit));
}

function forEachAction(pages, visit) {
  for (const p of pages) {
    for (const b of Object.values(p.keys)) {
      for (const key of ['action', 'double', 'hold', 'left', 'right', 'press_left', 'press_right']) actionLists(b[key], (a) => visit(a, p));
    }
  }
}

const allPages = () => state.cfg.profile.flatMap((p) => p.page);
/** The key a Change look or Reset look points at, as stored: left-out parts mean "this one". */
const lookTarget = (a) => a.set_look ?? a.reset_look;

function usesOfPage(name) {
  const uses = [];
  forEachAction(profile().page, (a, p) => { if (a.page === name || a.show === name) uses.push(p.name); });
  return [...new Set(uses)];
}

function renamePage(name) {
  const p = page();
  if (!name || name === p.name) return draw();
  if (profile().page.some((o) => o.name === name)) {
    notify(`This profile already has a page named ${name}.`, true);
    return draw();
  }
  const old = p.name;
  forEachAction(profile().page, (a) => {
    if (a.page === old) a.page = name;
    if (a.show === old) a.show = name;
  });
  for (const owner of state.cfg.profile) {
    forEachAction(owner.page, (a) => {
      const t = lookTarget(a);
      if (t?.page === old && (t.profile ?? owner.name) === profile().name) t.page = name;
    });
  }
  p.name = name;
  save();
  draw();
}

function deletePage() {
  if (!page().folder && profile().page.filter((p) => !p.folder).length === 1) return notify('A profile needs at least one page.', true);
  const uses = usesOfPage(page().name);
  if (uses.length) return notify(`Still used by: ${uses.join(', ')}. Remove those first.`, true);
  confirmDialog(`Delete ${page().name}?`, 'Everything set on this page goes with it.', 'Delete page', () => {
    profile().page.splice(state.page, 1);
    state.page = Math.max(0, state.page - 1);
    state.selected = null;
    save();
    draw();
  });
}

function duplicatePage() {
  const pages = profile().page;
  if (page().folder) return addFolderCopy();
  let name = ` copy`;
  while (pages.some((p) => p.name === name)) name += ' copy';
  pages.splice(state.page + 1, 0, { ...structuredClone(page()), name });
  state.page += 1;
  state.selected = null;
  save();
  draw();
}

function addFolderCopy() {
  const pages = profile().page;
  let name = ` copy`;
  while (pages.some((p) => p.name === name)) name += ' copy';
  pages.push({ ...structuredClone(page()), name });
  state.page = pages.length - 1;
  state.selected = null;
  save();
  draw();
}

function startRename() {
  const tab = document.querySelector('.tab[aria-current="true"]');
  if (!tab) return;
  let cancelled = false;
  const input = h('input', {
    class: 'tab-input', value: page().name, 'aria-label': 'Page name',
    onkeydown: (e) => {
      if (e.key === 'Enter') input.blur();
      if (e.key === 'Escape') { e.stopPropagation(); cancelled = true; input.blur(); }
    },
    onblur: () => (cancelled ? draw() : renamePage(input.value.trim())),
  });
  input.setAttribute('type', 'text');
  tab.replaceWith(input);
  input.select();
}

function pageMenu(at, index) {
  state.page = index;
  state.selected = null;
  draw();
  const p = page();
  openMenu(at, [
    { label: 'Rename', icon: 'pencil-simple-bold', run: startRename },
    { label: 'Duplicate', icon: 'copy-bold', run: duplicatePage },
    p.folder ? null : { label: 'Skip in Next and Previous page', icon: 'skip-forward-bold', checked: !!p.hidden, run: () => { p.hidden = !p.hidden; save(); draw(); } },
    '-',
    { label: p.folder ? 'Delete folder' : 'Delete page', icon: 'trash-bold', danger: true, run: deletePage },
  ]);
}

function byKind() {
  // Normal pages first: a profile opens on its first page, which can't be a folder.
  profile().page.sort((a, b) => !!a.folder - !!b.folder);
}

function freeName(base) {
  let n = profile().page.length + 1;
  while (profile().page.some((p) => p.name === `${base} ${n}`)) n++;
  return `${base} ${n}`;
}

function newFolder() {
  return { name: freeName('Folder'), folder: true, keys: {} };
}

function addFolder() {
  const folder = newFolder();
  profile().page.push(folder);
  state.page = profile().page.indexOf(folder);
  state.selected = null;
  save();
  draw();
}

/** A new page starts with the first page's knobs and side buttons. */
function addPage() {
  const pages = profile().page;
  const keys = {};
  for (const control of [...KNOBS, 'side1', 'side2']) {
    if (pages[0].keys[control]) keys[control] = structuredClone(pages[0].keys[control]);
  }
  const fresh = { name: freeName('Page'), keys };
  pages.push(fresh);
  byKind();
  state.page = pages.indexOf(fresh);
  state.selected = null;
  save();
  draw();
}

function pageTabs() {
  const tabs = profile().page.map((p, i) => {
    const tab = h('button', {
      class: p.hidden ? 'tab skipped' : 'tab',
      'aria-current': String(i === state.page),
      onclick: () => { state.page = i; state.selected = null; draw(); },
      ondblclick: startRename,
      oncontextmenu: (e) => { e.preventDefault(); e.stopPropagation(); pageMenu(menuPoint(e), i); },
    }, p.folder ? icon('folder-bold') : null, p.name);
    return tab;
  });
  return h('div', { class: 'tabs', role: 'group', 'aria-label': 'Pages' }, ...tabs, iconButton('plus-bold', 'Add page', addPage), iconButton('folder-plus-bold', 'Add folder', addFolder));
}

function defaultProfile() {
  const list = state.cfg.profile;
  const marked = list.findIndex((p) => p.default);
  if (marked >= 0) return marked;
  return Math.max(0, list.findIndex((p) => !p.exe && !p.title && !p.title_regex));
}

function setDefault(index) {
  state.cfg.profile.forEach((p, i) => { p.default = i === index; });
  save();
  drawProfiles();
}

function profileMenu(at, index) {
  const last = state.cfg.profile.length - 1;
  openMenu(at, [
    { label: 'Profile settings', icon: 'sliders-horizontal-bold', run: () => profileDialog(index) },
    { label: 'Set as default', icon: 'house-bold', checked: index === defaultProfile(), run: () => setDefault(index) },
    { label: 'Move up', icon: 'arrow-up-bold', disabled: index === 0, run: () => moveProfile(index, index - 1) },
    { label: 'Move down', icon: 'arrow-down-bold', disabled: index === last, run: () => moveProfile(index, index + 1) },
    '-',
    { label: 'Delete profile', icon: 'trash-bold', danger: true, run: () => deleteProfile(index) },
  ]);
}

function legend() {
  const entry = (kinds, text) => h('span', {}, ...kinds.map((kind) => mark(kind, true)), text);
  return h('p', { class: 'legend' }, entry(['tap'], 'Click'), entry(['double'], 'Double click'), entry(['long'], 'Hold'), entry(['left', 'right'], 'Turn'), entry(['press-left', 'press-right'], 'Turn while pressed'));
}

function drawKeysView() {
  const more = iconButton('dots-three-bold', 'Profile options', (e) => profileMenu(menuPoint(e), state.profile));
  const head = h('div', { class: 'bench-head' }, h('div', { class: 'profile-bar' }, h('h1', {}, profile().name), more), pageTabs());
  const bench = h('div', {
    class: 'bench',
    onclick: (e) => { if (e.target === bench) select(null); },
  }, head, h('div', { class: 'deck' }), legend());
  $('stage').replaceChildren(bench);
  drawDeck();
}

// ---------------------------------------------------------------- action list
function drawActions() {
  let index = 0;
  const entries = [];
  const groups = ACTION_GROUPS.map(([name, items]) => {
    const rows = items.map((item) => {
      const position = index++;
      if (item.knob && !hasDials()) return null;
      if (item.obs && !state.status?.obs?.connected) return null;
      const row = h('button', {
        class: item.hold ? 'action hold' : 'action',
        draggable: true,
        onclick: () => applyPalette(item, state.selected),
      }, icon(item.icon), item.name);
      row.addEventListener('dragstart', (e) => e.dataTransfer.setData('text/plain', `palette:${position}`));
      return row;
    }).filter(Boolean);
    if (!rows.length) return null;
    const group = h('details', {
      class: 'group',
      open: !state.collapsed.has(name),
      ontoggle: () => {
        if (search.value) return; // searching opens every group; that isn't the user's choice
        if (group.open) state.collapsed.delete(name);
        else state.collapsed.add(name);
        remember('collapsed', JSON.stringify([...state.collapsed]));
      },
    }, h('summary', {}, name), ...rows);
    entries.push({ name, group, rows });
    return group;
  });
  const empty = h('p', { class: 'hint', hidden: true }, 'No action matches that search.');
  const search = h('input', {
    type: 'search', placeholder: 'Search actions', 'aria-label': 'Search actions',
    oninput: () => {
      const words = search.value.toLowerCase().split(/\s+/).filter(Boolean);
      let shown = 0;
      for (const entry of entries) {
        const inGroup = words.every((w) => entry.name.toLowerCase().includes(w));
        const visible = entry.rows.filter((row) => {
          const match = inGroup || words.every((w) => row.textContent.toLowerCase().includes(w));
          row.hidden = !match;
          return match;
        }).length;
        entry.group.hidden = visible === 0;
        entry.group.open = words.length ? true : !state.collapsed.has(entry.name);
        shown += visible;
      }
      empty.hidden = shown > 0;
    },
  });
  $('actions').replaceChildren(h('div', { class: 'actions-search' }, search), empty, ...groups.filter(Boolean));
}

// ---------------------------------------------------------------- dialogs
function dialogHead(title) {
  return h('header', { class: 'dialog-head' }, h('h2', {}, title), iconButton('x-bold', 'Close dialog', () => $('dialog').close()));
}

function dialogFoot(...buttons) {
  return h('footer', { class: 'dialog-foot' }, ...buttons);
}

function openDialog(...content) {
  const dialog = $('dialog');
  dialog.classList.remove('narrow', 'medium');
  dialog.replaceChildren(...content);
  if (!dialog.open) dialog.showModal();
  return dialog;
}

function iconPicker(onPick) {
  const names = [...state.icons.keys()];
  const search = h('input', { type: 'search', placeholder: 'Search icons in English', 'aria-label': 'Search icons' });
  const style = h('select', { 'aria-label': 'Icon style' }, h('option', { value: 'bold' }, 'Bold'), h('option', { value: 'fill' }, 'Filled'));
  const grid = h('div', { class: 'icon-grid' });
  const count = h('p', { class: 'hint' });
  const LIMIT = 240;
  const refresh = () => {
    const words = search.value.toLowerCase().split(/\s+/).filter(Boolean);
    const found = names.filter((n) => n.endsWith(`-${style.value}`) && words.every((w) => n.includes(w)));
    grid.replaceChildren(...found.slice(0, LIMIT).map((name) => h('button', {
      class: 'icon-choice',
      title: name.replace(/-(bold|fill)$/, ''),
      onclick: () => { $('dialog').close(); onPick(`ph:${name}`); },
    }, icon(name))));
    count.textContent = found.length > LIMIT ? `Showing ${LIMIT} of ${found.length}. Search to narrow it down.` : `${found.length} icons`;
  };
  search.addEventListener('input', refresh);
  style.addEventListener('change', refresh);
  const own = h('button', {
    class: 'plain',
    onclick: async () => {
      const path = await invoke('pick_file', { name: 'Icons', extensions: ['svg', 'png'] });
      if (path) { $('dialog').close(); onPick(path); }
    },
  }, 'Use my own SVG or PNG');
  openDialog(
    dialogHead('Choose an icon'),
    section('Search', h('div', { class: 'row' }, search, style, own)),
    section('Icons', grid, count),
    dialogFoot(h('button', { class: 'plain', onclick: () => $('dialog').close() }, 'Cancel')),
  );
  refresh();
  search.focus();
}

/** Shows the deck of any page to click the key to change. Calls back with only what differs from here. */
function keyPicker(current, onPick) {
  let chosen = state.cfg.profile.find((p) => p.name === current.profile) ?? profile();
  let shown = chosen.page.find((p) => p.name === current.page) ?? (chosen === profile() && current.page == null ? page() : chosen.page[0]);
  const profiles = h('select', { 'aria-label': 'Profile' }, ...state.cfg.profile.map((p) => h('option', { value: p.name, selected: p === chosen }, p.name)));
  const pages = h('select', { 'aria-label': 'Page' });
  const grid = h('div', { class: 'mini-deck key-pick' });
  const pick = (control) => {
    $('dialog').close();
    const here = chosen === profile();
    onPick({
      profile: here ? undefined : chosen.name,
      page: here && shown === page() ? undefined : shown.name,
      key: here && shown === page() && control === state.selected ? undefined : control,
    });
  };
  const refresh = () => {
    pages.replaceChildren(...chosen.page.map((p) => h('option', { value: p.name, selected: p === shown }, p.name)));
    grid.replaceChildren(...SCREENS.map((c) => {
      const marked = c === (current.key ?? state.selected) && shown.name === (current.page ?? page().name) && chosen.name === (current.profile ?? profile().name);
      const cell = h('button', { type: 'button', class: `mini-key${c === 'wide' ? ' wide' : ''}${marked ? ' marked' : ''}`, title: controlName(c), 'aria-label': controlName(c), onclick: () => pick(c) });
      keyImage(visualOf(shown.keys[c], c), c === 'wide').then((url) => cell.replaceChildren(h('img', { src: url, alt: '' })));
      return cell;
    }));
  };
  profiles.onchange = () => { chosen = state.cfg.profile.find((p) => p.name === profiles.value); shown = chosen.page[0]; refresh(); };
  pages.onchange = () => { shown = chosen.page.find((p) => p.name === pages.value); refresh(); };
  openDialog(
    dialogHead('Choose the key to change'),
    section('Page', h('div', { class: 'row' }, profiles, pages)),
    section('Key', grid),
    dialogFoot(h('button', { class: 'plain', onclick: () => $('dialog').close() }, 'Cancel')),
  );
  refresh();
}

const WIDE_ASPECT = 458 / 196;
const PICTURE = /\.(png|jpe?g|gif|webp)$/i;

/**
 * Drag the frame over the picture to pick the part shown; the slider sizes it. The frame keeps the
 * shape of the key, so the picture is never squashed. Calls back with [x, y, width, height] as
 * fractions of the picture, or undefined for the whole picture.
 */
async function cropPicker(path, aspect, current, onPick) {
  let url;
  try {
    url = URL.createObjectURL(new Blob([await invoke('picture', { path })]));
  } catch (e) {
    return notify(`Cannot open the picture: ${e}`, true);
  }
  const img = h('img', { src: url, alt: '', draggable: false });
  const frame = h('div', { class: 'crop-frame' });
  const stage = h('div', { class: 'crop-stage' }, img, frame);
  const size = h('input', { type: 'range', min: 10, max: 100, value: 100, 'aria-label': 'Size' });
  let box = { x: 0, y: 0, w: 0, h: 0 };
  // The largest frame of this shape that fits the picture as shown.
  const largest = () => {
    const [w, h] = [img.clientWidth, img.clientHeight];
    return w / h > aspect ? { w: h * aspect, h } : { w, h: w / aspect };
  };
  const place = (x, y) => {
    box.x = Math.min(Math.max(0, x), img.clientWidth - box.w);
    box.y = Math.min(Math.max(0, y), img.clientHeight - box.h);
    Object.assign(frame.style, { left: `${box.x}px`, top: `${box.y}px`, width: `${box.w}px`, height: `${box.h}px` });
  };
  const resize = () => {
    const max = largest();
    const [cx, cy] = [box.x + box.w / 2, box.y + box.h / 2];
    box.w = (max.w * size.value) / 100;
    box.h = (max.h * size.value) / 100;
    place(cx - box.w / 2, cy - box.h / 2);
  };
  size.oninput = resize;
  frame.addEventListener('pointerdown', (e) => {
    e.preventDefault();
    frame.setPointerCapture(e.pointerId);
    const [startX, startY, fromX, fromY] = [e.clientX, e.clientY, box.x, box.y];
    frame.onpointermove = (m) => place(fromX + m.clientX - startX, fromY + m.clientY - startY);
    frame.onpointerup = () => { frame.onpointermove = null; };
  });
  const done = (crop) => {
    $('dialog').close();
    URL.revokeObjectURL(url);
    onPick(crop);
  };
  const use = () => {
    const [w, h] = [img.clientWidth, img.clientHeight];
    // Rounded down, so the crop can never reach past the picture's edge.
    const f = (v) => Math.floor(v * 10000) / 10000;
    done([f(box.x / w), f(box.y / h), f(box.w / w), f(box.h / h)]);
  };
  openDialog(
    dialogHead('Choose the part to show'),
    section('Picture', stage, field('Size', size)),
    dialogFoot(
      h('button', { class: 'plain', onclick: () => done(undefined) }, 'Use the whole picture'),
      h('button', { class: 'plain', onclick: () => $('dialog').close() }, 'Cancel'),
      h('button', { class: 'plain primary', onclick: use }, 'Use this part')),
  );
  $('dialog').classList.add('medium');
  await img.decode().catch(() => {});
  const max = largest();
  if (current) {
    const [x, y, w] = current;
    size.value = Math.round(((w * img.clientWidth) / max.w) * 100);
    box = { x: 0, y: 0, w: w * img.clientWidth, h: (w * img.clientWidth) / aspect };
    place(x * img.clientWidth, y * img.clientHeight);
  } else {
    box = { x: 0, y: 0, ...max };
    place((img.clientWidth - max.w) / 2, (img.clientHeight - max.h) / 2);
  }
}

function keyboardPicker(current, onSave) {
  const tokens = current ? current.split('+').map((t) => t.trim().toLowerCase()).filter(Boolean) : [];
  const preferred = recall('keyboard') ?? (navigator.language === 'pt-BR' ? 'Brazilian (ABNT2)' : 'US (ANSI)');
  const layout = h('select', { 'aria-label': 'Keyboard layout' }, ...Object.keys(LAYOUTS).map((n) => h('option', { value: n, selected: n === preferred }, n)));
  const combo = h('p', { class: 'combo' });
  const board = h('div', { class: 'board' });

  const toggle = (token) => {
    const at = tokens.indexOf(token);
    if (at >= 0) tokens.splice(at, 1);
    else tokens.push(token);
    refresh();
  };
  const keyRow = (keys) => h('div', { class: 'kb-row' }, ...keys.map(([label, token, width]) => (label === null
    ? h('span', { class: 'kb-gap', style: `--w:${width}` })
    : h('button', { class: 'kb-key', style: `--w:${width}`, 'aria-pressed': String(tokens.includes(token)), onclick: () => toggle(token) }, label))));
  const hex = h('input', { type: 'text', maxLength: 2, placeholder: 'EA', 'aria-label': 'Key code in hex' });
  const addHex = () => {
    const code = parseInt(hex.value, 16);
    if (!code || code > 255) return notify('A key code is a hex number from 01 to FF.', true);
    toggle(`vk${code.toString(16).padStart(2, '0')}`);
    hex.value = '';
  };
  function refresh() {
    combo.textContent = tokens.length ? tokens.join(' + ') : 'Click keys in the order they should be pressed';
    remember('keyboard', layout.value);
    board.replaceChildren(
      h('div', { class: 'kb-block' }, ...FUNCTION_ROWS.map(keyRow), ...LAYOUTS[layout.value].map(keyRow)),
      h('div', { class: 'kb-side' },
        h('div', { class: 'kb-block small' }, ...NAV_BLOCK.map(keyRow)),
        h('div', { class: 'kb-block small' }, ...NUMPAD.map(keyRow))),
      h('div', { class: 'kb-media' }, keyRow(MEDIA_KEYS)),
    );
  }
  layout.addEventListener('change', refresh);
  openDialog(
    dialogHead('Choose keys'),
    section('Selected keys', combo),
    section('Keyboard', h('div', { class: 'row kb-options' }, layout, h('span', { class: 'kb-hex' }, 'Other key code'), hex, h('button', { onclick: addHex }, 'Add')), board),
    dialogFoot(
      h('button', { class: 'plain', onclick: () => { tokens.length = 0; refresh(); } }, 'Clear'),
      h('button', { class: 'plain', onclick: () => $('dialog').close() }, 'Cancel'),
      h('button', { class: 'plain primary', onclick: () => { $('dialog').close(); onSave(tokens.join('+')); } }, 'Use these keys')),
  );
  refresh();
}

// ---------------------------------------------------------------- inspector
const ACTION_TYPES = [
  ['none', 'Nothing'], ['hotkey', 'Hotkey'], ['text', 'Type text'], ['open', 'Open app, file or website'], ['close', 'Close app'],
  ['page', 'Open page'], ['back', 'Go back'], ['profile', 'Switch profile'], ['next_page', 'Next page'], ['prev_page', 'Previous page'],
  ['scroll_up', 'Scroll up'], ['scroll_down', 'Scroll down'], ['scroll_left', 'Scroll left'], ['scroll_right', 'Scroll right'],
  ['set_look', 'Change look'], ['reset_look', 'Reset look'], ['block_input', 'Block input'], ['unblock_input', 'Unblock input'], ['brightness', 'Change deck brightness'], ['app_volume', 'Change app volume'], ['app_volume_reset', 'Reset app volume'], ['play_sound', 'Play sound file'], ['stop_sound', 'Stop sound'],
  ['sequence', 'Sequence'], ['toggle', 'Toggle between actions'], ['random', 'Random action'],
  ['obs_toggle_record', 'OBS: Record'], ['obs_toggle_record_pause', 'OBS: Pause recording'], ['obs_toggle_stream', 'OBS: Stream'],
  ['obs_toggle_virtual_cam', 'OBS: Virtual camera'], ['obs_toggle_replay_buffer', 'OBS: Replay buffer'], ['obs_save_replay', 'OBS: Save replay'],
  ['obs_toggle_studio_mode', 'OBS: Studio mode'], ['obs_transition', 'OBS: Transition'], ['obs_scene_transition', 'OBS: Scene transition effect'],
  ['obs_screenshot', 'OBS: Screenshot'], ['obs_scene_collection', 'OBS: Scene collection'], ['obs_profile', 'OBS: Profile'], ['obs_scene', 'OBS: Scene'],
  ['obs_toggle_source', 'OBS: Source visibility'], ['obs_chapter', 'OBS: Chapter marker'], ['obs_toggle_filter', 'OBS: Filter'],
  ['obs_media', 'OBS: Media source control'], ['obs_toggle_mute', 'OBS: Mute input'], ['obs_volume', 'OBS: Input volume'],
];
const DELAY_TYPE = ['delay', 'Wait'];
const SHOW_TYPE = ['show', 'Show a page while held'];
const actionType = (a) => (!a ? 'none' : typeof a === 'string' ? a : a.obs ? `obs_${actionType(a.obs)}` : a.plugin ? `plugin:${a.plugin.id}:${a.plugin.action}` : Object.keys(a)[0]);

const ACTION_NAMES = Object.fromEntries([...ACTION_TYPES, DELAY_TYPE, SHOW_TYPE]);

/** The action a list entry produces, so filled slots can reuse its icon. */
function paletteResult(item) {
  const result = {};
  item.apply(result, context);
  return result;
}

function actionIcon(type) {
  const item = PALETTE.find((entry) => {
    const { action, hold } = paletteResult(entry);
    return actionType(hold?.show === undefined ? action : hold) === type && !entry.knob;
  });
  return item ? icon(item.icon) : null;
}

/** Turns a list entry into a slot's value, or explains why it doesn't fit there. */
function fillSlot(item, options, commit, decorate) {
  ensureFolder(item);
  const result = paletteResult(item);
  if (item.knob || item.screen) return notify(` cannot go in an action slot. Drop it on a control.`, true);
  if (item.wait && !options.delay) return notify('Wait only works inside a sequence.', true);
  if (result.hold !== undefined) {
    if (result.hold.show === undefined) return notify('Drag the action to run into this slot.', true);
    if (!options.show) return notify('Showing a page only works in the When held slot.', true);
    decorate?.(result);
    return commit(result.hold);
  }
  decorate?.(result);
  commit(result.action);
}

function pageOptions(selected, extra = []) {
  return [...extra, ...profile().page.map((p) => [p.name, p.name])]
    .map(([value, text]) => h('option', { value, selected: value === selected }, text));
}

function browseButton(extensions, commit) {
  return h('button', {
    type: 'button',
    onclick: async () => {
      const path = await invoke('pick_file', { name: 'Files', extensions });
      if (path) commit(path);
    },
  }, 'Browse');
}

let nameLists = 0;

/** Suggests the names reported by OBS or a plugin, but takes any text: it may be closed while editing. */
function nameField(label, value, names, commit, hint) {
  const id = `names-${++nameLists}`;
  const input = h('input', { type: 'text', value, list: id, onchange: () => commit(input.value) });
  return h('div', {}, field(label, input, hint), h('datalist', { id }, ...(names ?? []).map((name) => h('option', { value: name }))));
}

function pluginFields(data, commit) {
  const plugin = state.plugins.find((p) => p.id === data.id);
  const action = plugin?.actions.find((a) => a.id === data.action);
  if (!action) return [h('p', { class: 'hint' }, 'This plugin is not installed, so the action does nothing.')];
  return action.fields.map((f) => nameField(f.label, data.params?.[f.key] ?? '', plugin.options[f.options],
    (text) => commit({ ...data, params: { ...data.params, [f.key]: text } }), f.hint));
}

/** The fields of an OBS action, which is `{ obs: 'name' }` or `{ obs: { name: value } }`. */
function obsFields(kind, value, commit) {
  const obs = state.status?.obs;
  const data = value[kind];
  const sources = [...(obs?.scenes ?? []), ...(obs?.inputs ?? [])];
  const change = (key) => (text) => commit({ [kind]: { ...data, [key]: text } });
  if (kind === 'toggle_record_pause') return [h('p', { class: 'hint' }, 'OBS cannot pause while its recording quality is Same as stream.')];
  const one = (label, names) => [nameField(label, data, names, (text) => commit({ [kind]: text }))];
  if (kind === 'scene') return one('Scene', obs?.scenes);
  if (kind === 'scene_collection') return one('Scene collection', obs?.collections);
  if (kind === 'profile') return one('Profile', obs?.profiles);
  if (kind === 'scene_transition') return one('Transition', obs?.transitions);
  if (kind === 'toggle_mute') return one('Input', obs?.inputs);
  if (kind === 'chapter') return [...one('Chapter name', []), h('p', { class: 'hint' }, 'Leave it empty to let OBS name it. Needs a Hybrid MP4 recording.')];
  if (kind === 'toggle_source') return [nameField('Scene', data.scene, obs?.scenes, change('scene')), nameField('Source', data.source, obs?.items?.[data.scene], change('source'))];
  if (kind === 'toggle_filter') return [nameField('Source', data.source, sources, change('source')), nameField('Filter', data.filter, obs?.filters?.[data.source], change('filter'))];
  if (kind === 'media') {
    const select = h('select', { onchange: () => change('action')(select.value) },
      ...[['play', 'Play'], ['pause', 'Pause'], ['stop', 'Stop'], ['restart', 'Restart'], ['next', 'Next'], ['previous', 'Previous']]
        .map(([id, text]) => h('option', { value: id, selected: id === data.action }, text)));
    return [nameField('Input', data.input, obs?.inputs, change('input')), field('Action', select)];
  }
  if (kind === 'volume') {
    const input = h('input', { type: 'number', value: data.db, min: -60, max: 60, onchange: () => commit({ volume: { ...data, db: Math.trunc(Number(input.value)) || 0 } }) });
    return [nameField('Input', data.input, obs?.inputs, change('input')), field('Change by (dB)', input, 'Stops at 0 dB, so it never boosts the input.')];
  }
  return [];
}

function listEditor(key, action, commit, inSequence) {
  let list = action[key];
  // Typing commits quietly, without a redraw, so later edits must start from the latest list.
  const update = (next, quiet) => { list = next; commit({ [key]: next }, quiet); };
  const move = (from, to) => {
    const next = [...list];
    next.splice(to, 0, next.splice(from, 1)[0]);
    update(next);
  };
  const mover = (i, to, name, glyph) => h('button', {
    type: 'button', class: 'slot-tool', title: name, 'aria-label': `Move step ${i + 1} ${name.toLowerCase()}`,
    disabled: to < 0 || to >= list.length,
    onclick: () => move(i, to),
  }, icon(glyph));
  const steps = list.map((step, i) => actionEditor(`Step ${i + 1}`, step, (value, quiet) => {
    const next = [...list];
    if (value === undefined) next.splice(i, 1);
    else next[i] = value;
    update(next, quiet);
  }, { delay: inSequence, tools: [mover(i, i - 1, 'Up', 'caret-up-bold'), mover(i, i + 1, 'Down', 'caret-down-bold')] }));
  const add = actionEditor('', undefined, (value) => update([...list, value]), { delay: inSequence, prompt: 'Drag an action here to add a step' });
  return h('div', { class: 'steps' }, ...steps, add);
}

/** A dotted slot that takes an action dragged from the list, or the next one clicked there. */
function actionEditor(label, action, commit, options = {}) {
  // One shot wraps the real action, so the editor below only ever sees the inner one.
  const plainCommit = commit;
  const shot = action?.one_shot !== undefined;
  if (shot) {
    action = action.one_shot;
    commit = (value, quiet) => plainCommit(value === undefined ? undefined : { one_shot: value }, quiet);
  }
  // Following an app is an option of the profile action, so it edits like one.
  const follow = action?.follow_app;
  if (follow) action = { profile: APP_LAYER };
  const type = actionType(action);
  const typeName = ACTION_NAMES[type] ?? action?.plugin?.action ?? type;
  const parts = [];
  const take = (item) => {
    state.armed = null;
    fillSlot(item, options, commit, options.decorate);
  };
  const slot = h('div', { class: action ? 'slot filled' : 'slot' });
  if (action) {
    slot.append(h('div', { class: 'slot-head' }, actionIcon(type), h('span', { class: 'slot-name' }, typeName),
      ...(options.tools ?? []),
      h('button', { type: 'button', class: 'slot-tool', title: 'Remove', 'aria-label': `Remove ${typeName}`, onclick: () => commit(undefined) }, icon('x-bold'))));
  } else {
    slot.append(h('button', {
      type: 'button', class: 'slot-empty',
      onclick: () => {
        const armed = state.armed === take;
        document.querySelectorAll('.slot.armed').forEach((el) => el.classList.remove('armed'));
        state.armed = armed ? null : take;
        slot.classList.toggle('armed', !armed);
        if (!armed) notify('Now click an action in the list.');
      },
    }, options.prompt ?? 'Drag an action here'));
  }
  slot.addEventListener('dragover', (e) => { e.preventDefault(); e.stopPropagation(); slot.classList.add('over'); });
  slot.addEventListener('dragleave', (e) => { e.stopPropagation(); slot.classList.remove('over'); });
  slot.addEventListener('drop', (e) => {
    e.preventDefault();
    e.stopPropagation();
    slot.classList.remove('over');
    const [kind, value] = e.dataTransfer.getData('text/plain').split(':');
    if (kind === 'palette') take(PALETTE[Number(value)]);
  });
  const textField = (key, placeholder) => {
    const input = h('input', { type: 'text', value: action[key], placeholder, oninput: () => commit({ [key]: input.value }, true) });
    return input;
  };
  if (type === 'hotkey') {
    const keys = action.hotkey ? action.hotkey.split('+').map((key) => h('kbd', {}, key.trim())) : ['Choose keys'];
    parts.push(h('button', {
      type: 'button', class: 'keys', title: 'Choose keys',
      onclick: () => keyboardPicker(action.hotkey, (value) => commit({ hotkey: value })),
    }, ...keys));
  } else if (type === 'text') {
    parts.push(textField('text', 'Text to type'));
  } else if (type === 'open') {
    parts.push(h('div', { class: 'row' }, textField('open', 'Path, folder, URL or command'), browseButton([], (path) => commit({ open: path }))));
    parts.push(h('p', { class: 'hint' }, 'Works like the Run dialog: notepad file.txt, ms-settings:, or shell:AppsFolder\<app id> for Store apps. Quote a path with spaces to give it arguments.'));
  } else if (type === 'close') {
    parts.push(textField('close', 'Program, like notepad.exe'));
    parts.push(h('p', { class: 'hint' }, 'Leave it empty to close the focused window.'));
  } else if (type === 'page') {
    const select = h('select', { onchange: () => commit({ page: select.value }) }, ...pageOptions(action.page));
    parts.push(select);
    parts.push(h('p', { class: 'hint' }, 'Put a Go back action on that page to return here.'));
  } else if (type === 'show') {
    const select = h('select', { 'aria-label': 'Page to show', onchange: () => commit({ show: select.value }) },
      ...pageOptions(action.show, [[APP_LAYER, 'Page of the focused app']]));
    parts.push(select);
  } else if (type === 'profile') {
    const select = h('select', { onchange: () => commit({ profile: select.value }) },
      ...[[APP_LAYER, "Focused app's profile"], [DEFAULT_PROFILE, 'Default profile'], ...state.cfg.profile.map((p) => [p.name, p.name])]
        .map(([value, text]) => h('option', { value, selected: value === action.profile }, text)));
    parts.push(select);
  } else if (type.startsWith('scroll_')) {
    const input = h('input', { type: 'number', value: action[type], min: 1, max: 100, onchange: () => commit({ [type]: Math.max(1, Number(input.value) || 1) }) });
    parts.push(field('Notches', input));
  } else if (type === 'app_volume') {
    const input = h('input', { type: 'number', value: action.app_volume, min: -100, max: 100, onchange: () => commit({ app_volume: Number(input.value) || 0 }) });
    parts.push(field('Change by (%)', input, 'Applies to the window that has focus.'));
  } else if (type === 'set_look' || type === 'reset_look') {
    // Values come back from the app as null when left out, so test with == null.
    const data = action[type];
    // Another profile has other pages, so picking one starts over at its first page.
    const change = (key, value) => commit({ [type]: { ...data, ...(key === 'profile' && { page: undefined }), [key]: value ?? undefined } });
    const where = data.key == null && data.page == null && data.profile == null ? 'This key'
      : [controlName(data.key ?? state.selected), data.page ?? (data.profile == null ? null : 'first page'), data.profile].filter((x) => x != null).join(' · ');
    const retarget = (t) => commit({ [type]: { ...data, profile: t.profile, page: t.page, key: t.key } });
    parts.push(field('Key to change', h('div', { class: 'row' },
      h('button', { type: 'button', onclick: () => keyPicker({ profile: data.profile ?? undefined, page: data.page ?? undefined, key: data.key ?? undefined }, retarget) }, where),
      where === 'This key' ? null : h('button', { type: 'button', class: 'plain', onclick: () => retarget({}) }, 'Use this key'))));
    if (type === 'set_look') {
      const label = h('input', { type: 'text', value: data.label ?? '', placeholder: 'Keep the label', onchange: () => change('label', label.value || undefined) });
      parts.push(field('Label', label));
      // Unticked colours and an empty tile leave that part of the key as it is.
      const swatch = (key, title, fallback) => {
        const input = h('input', { type: 'color', value: data[key] ?? fallback, disabled: data[key] == null, 'aria-label': title, onchange: () => change(key, input.value) });
        const on = h('input', { type: 'checkbox', checked: data[key] != null, 'aria-label': `Change ${title.toLowerCase()}`, onchange: () => change(key, on.checked ? input.value : undefined) });
        return h('label', {}, on, input, title);
      };
      // A crop belongs to its picture, so picking another picture drops it.
      const picture = (key, crop) => (value) => commit({ [type]: { ...data, [key]: value ?? undefined, [crop]: undefined } });
      const wide = (data.key ?? (data.page == null && data.profile == null ? state.selected : null)) === 'wide';
      parts.push(h('div', { class: 'looks' }, iconTile(data.icon ?? undefined, picture('icon', 'icon_crop'), 'Keep the current icon', { value: data.icon_crop, set: (crop) => change('icon_crop', crop) }),
        h('div', { class: 'swatches' }, swatch('color', 'Icon', state.cfg.color), swatch('background', 'Background', state.cfg.background))));
      const image = h('input', { type: 'text', value: data.image ?? '', placeholder: 'Keep the image', 'aria-label': 'Background image', onchange: () => picture('image', 'image_crop')(image.value || undefined) });
      const crop = data.image ? h('button', { type: 'button', onclick: () => cropPicker(data.image, wide ? WIDE_ASPECT : 1, data.image_crop, (c) => change('image_crop', c)) }, 'Crop') : null;
      parts.push(field('Background image', h('div', { class: 'row' }, image, browseButton(['png', 'jpg', 'jpeg', 'gif', 'webp'], picture('image', 'image_crop')), crop)));
      parts.push(h('p', { class: 'hint' }, 'Lasts until the config is saved again. Anything left empty or unticked stays as it is.'));
    } else {
      parts.push(h('p', { class: 'hint' }, 'Undoes every Change look on that key.'));
    }
  } else if (type === 'next_page' || type === 'prev_page') {
    // Wrapping is the default and is saved as the plain name.
    const box = h('input', { type: 'checkbox', checked: action[type]?.wrap !== false, onchange: () => commit(box.checked ? type : { [type]: { wrap: false } }) });
    parts.push(h('label', { class: 'check' }, box, type === 'next_page' ? 'After the last page, go to the first' : 'Before the first page, go to the last'));
  } else if (type === 'block_input') {
    parts.push(h('p', { class: 'hint' }, 'The deck ignores every button until one with Unblock input runs it, e.g. this key when held.'));
  } else if (type === 'unblock_input') {
    parts.push(h('p', { class: 'hint' }, 'Works even while the deck is blocked.'));
  } else if (type === 'app_volume_reset') {
    parts.push(h('p', { class: 'hint' }, 'Sets the focused app to 100%, so it follows the system volume again.'));
  } else if (type === 'brightness') {
    const input = h('input', { type: 'number', value: action.brightness, min: -100, max: 100, onchange: () => commit({ brightness: Number(input.value) || 0 }) });
    parts.push(field('Change by', input));
  } else if (type === 'play_sound') {
    parts.push(h('div', { class: 'row' }, textField('play_sound', 'WAV or MP3 file'), browseButton(['wav', 'mp3'], (path) => commit({ play_sound: path }))));
  } else if (type === 'delay') {
    const input = h('input', { type: 'number', value: action.delay, min: 0, step: 50, onchange: () => commit({ delay: Number(input.value) || 0 }) });
    parts.push(field('Milliseconds', input));
  } else if (type === 'sequence') {
    parts.push(listEditor('sequence', action, commit, true));
  } else if (type === 'toggle' || type === 'random') {
    parts.push(h('p', { class: 'hint' }, type === 'toggle' ? 'Each press runs the next step.' : 'Each press runs one step at random.'));
    parts.push(listEditor(type, action, commit, false));
  } else if (type.startsWith('obs_')) {
    parts.push(...obsFields(type.slice(4), action.obs, (obs) => commit({ obs })));
  } else if (action?.plugin) {
    parts.push(...pluginFields(action.plugin, (plugin) => commit({ plugin })));
  }
  if (action?.profile === APP_LAYER && !shot) {
    const until = h('input', {
      type: 'checkbox', checked: !!follow,
      onchange: () => commit(until.checked ? { follow_app: { others: false } } : { profile: APP_LAYER }),
    });
    parts.push(h('label', { class: 'check one-shot' }, until, 'Return when the app loses focus'));
    if (follow) {
      const others = h('input', { type: 'checkbox', checked: !!follow.others, onchange: () => commit({ follow_app: { others: others.checked } }) });
      parts.push(h('label', { class: 'check one-shot nested' }, others, 'Follow other apps that have a profile'));
    }
  }
  if ((type === 'page' || type === 'profile') && !follow) {
    const box = h('input', { type: 'checkbox', checked: shot, onchange: () => plainCommit(box.checked ? { one_shot: action } : action) });
    parts.push(h('label', { class: 'check one-shot', title: 'Goes back after the next button press' }, box, 'One shot'));
    if (shot) parts.push(h('p', { class: 'hint' }, 'Returns here after the next button press.'));
  }
  slot.append(...parts);
  return h('div', { class: 'slot-field' }, label ? h('span', { class: 'slot-label' }, label) : null, slot);
}

/** `quiet` keeps the input focused while typing; the deck still repaints. */
function setter(control, key) {
  return (value, quiet) => {
    edit(control, (b) => { b[key] = value; });
    if (!quiet) drawInspector();
  };
}

function displaySection(control, b) {
  const set = setter(control, 'display');
  if (control === 'wide') {
    const current = wideDisplay(b);
    const select = h('select', { onchange: () => set(select.value) },
      ...WIDE_DISPLAYS.map(([value, text]) => h('option', { value, selected: value === current }, text)));
    return field('Show', select);
  }
  const box = h('input', { type: 'checkbox', checked: b?.display === 'page_indicator', onchange: () => set(box.checked ? 'page_indicator' : undefined) });
  return h('label', { class: 'check' }, box, 'Show the page number');
}

/** `crop`, when given, offers cropping a picture icon: { value, set }. */
function iconTile(value, setIcon, removeLabel = 'Remove icon', crop = null) {
  const file = () => invoke('pick_file', { name: 'Icons', extensions: ['svg', 'png'] }).then((path) => { if (path) setIcon(path); });
  const library = value?.startsWith('ph:');
  const face = library ? icon(value.slice(3)) : value ? 'File' : icon('plus-bold');
  const tile = h('button', {
    type: 'button',
    class: `icon-tile${value ? (library ? '' : ' file') : ' empty'}`,
    title: value && !library ? value : 'Choose icon',
    'aria-label': 'Choose icon',
    onclick: () => iconPicker((value) => setIcon(value)),
    oncontextmenu: (e) => {
      e.preventDefault();
      e.stopPropagation();
      openMenu(menuPoint(e), [
        { label: 'Choose icon', icon: 'image-bold', run: () => iconPicker((value) => setIcon(value)) },
        { label: 'Use my own file', icon: 'folder-open-bold', run: file },
        crop && PICTURE.test(value ?? '') && { label: 'Crop picture', icon: 'crop-bold', run: () => cropPicker(value, 1, crop.value, crop.set) },
        '-',
        { label: removeLabel, icon: 'x-bold', danger: true, disabled: !value, run: () => setIcon(undefined) },
      ]);
    },
  }, face);
  return tile;
}

function appearance(control, b) {
  const set = setter(control, 'label');
  const label = h('input', { type: 'text', value: b?.label ?? '', 'aria-label': 'Name', oninput: () => set(label.value, true) });
  if (!hasScreen(control)) return [label];

  const parts = [displaySection(control, b)];
  // A new picture makes the old crop meaningless.
  const setPicture = (key, crop) => (value) => {
    edit(control, (x) => { x[key] = value; delete x[crop]; });
    drawInspector();
  };
  const aspect = control === 'wide' ? WIDE_ASPECT : 1;
  const imageField = () => {
    const setImage = setPicture('image', 'image_crop');
    const image = h('input', { type: 'text', value: b?.image ?? '', placeholder: 'None', 'aria-label': 'Background image', onchange: () => setImage(image.value || undefined) });
    const crop = b?.image ? h('button', { type: 'button', onclick: () => cropPicker(b.image, aspect, b.image_crop, setter(control, 'image_crop')) }, 'Crop') : null;
    return field('Background image', h('div', { class: 'row' }, image, browseButton(['png', 'jpg', 'jpeg', 'gif', 'webp'], setImage), crop));
  };
  if (control === 'wide' && !['image', 'page_indicator'].includes(wideDisplay(b))) {
    parts.push(imageField());
    parts.push(h('p', { class: 'hint' }, 'The deck draws this face itself, over the background image. Its colours cannot be changed, and the label and icon do not apply.'));
    return parts;
  }
  const swatch = (key, title, fallback) => {
    // Dragging only previews here; the deck gets the colour when the picker settles.
    const input = h('input', {
      type: 'color', value: b?.[key] ?? fallback, 'aria-label': title,
      oninput: () => edit(control, (x) => { x[key] = input.value; }, false),
      onchange: save,
    });
    return h('label', {}, input, title);
  };
  parts.push(h('div', { class: 'looks' }, iconTile(b?.icon, setPicture('icon', 'icon_crop'), 'Remove icon', { value: b?.icon_crop, set: setter(control, 'icon_crop') }),
    h('div', { class: 'swatches' }, swatch('color', 'Icon', state.cfg.color), swatch('background', 'Background', state.cfg.background))));
  parts.push(field('Label', label));
  parts.push(imageField());
  return parts;
}

function holdSection(control, b) {
  const parts = [actionEditor('', b?.hold, setter(control, 'hold'), { show: true, decorate: decorator(control) })];
  if (!b?.hold) return parts;
  const shows = b.hold.show !== undefined;
  const instant = shows && !b.action && !b.double;
  if (instant) {
    parts.push(h('p', { class: 'hint' }, 'The page appears as soon as this is pressed, and goes away on release.'));
    return parts;
  }
  const setHold = setter(control, 'hold_ms');
  const fallback = shows ? state.cfg.hold_ms : state.cfg.hold_action_ms;
  const time = h('input', {
    type: 'number', min: 50, max: 5000, step: 50, value: b.hold_ms ?? fallback,
    onchange: () => setHold(Number(time.value) || null, true),
  });
  parts.push(field('Hold for (ms)', time, shows
    ? 'A quick tap runs the click action. Holding longer, or pressing another key meanwhile, shows the page.'
    : 'A border draws around the key, then the action runs. Letting go early cancels it.'));
  return parts;
}

/** Dropping an action also fills in its default label and icon if the control has none. */
function decorator(control) {
  return (result) => edit(control, (b) => {
    b.label ||= result.label;
    b.icon ||= result.icon;
  });
}

function drawInspector() {
  state.armed = null;
  const root = $('inspector');
  const control = state.selected;
  if (state.view !== 'keys') return root.replaceChildren();
  if (state.selection.size > 1) {
    const targets = selectedList();
    return root.replaceChildren(
      h('div', { class: 'inspector-head' }, h('h2', {}, `${targets.length} controls selected`), iconButton('x-bold', 'Close', () => select(null))),
      h('p', { class: 'hint' }, 'Settings are edited one control at a time. Ctrl+C copies these, Ctrl+V pastes onto them, and Delete clears them.'),
      h('div', { class: 'row' },
        h('button', { type: 'button', onclick: () => copyControls(targets) }, 'Copy'),
        h('button', { type: 'button', disabled: !state.clipboard, onclick: () => pasteControls(targets) }, 'Paste'),
        h('button', { type: 'button', class: 'danger', onclick: () => clearControls(targets) }, 'Clear')));
  }
  if (control === null) {
    return root.replaceChildren(h('p', { class: 'panel-empty' }, 'Select a key, side button or knob to set it up. Ctrl+click or Shift+click selects several.'));
  }

  const b = binding(control);
  const parts = [
    h('div', { class: 'inspector-head' }, h('h2', {}, controlName(control)),
      iconButton('dots-three-bold', 'More options', (e) => controlMenu(menuPoint(e), control)),
      iconButton('x-bold', 'Close', () => select(null))),
    section(hasScreen(control) ? 'Appearance' : 'Name', ...appearance(control, b)),
  ];
  const slot = (key, label, options = {}) => actionEditor(label, b?.[key], setter(control, key), options);
  const doubleHint = b?.double ? h('p', { class: 'hint' }, `The single click runs ${state.cfg.double_ms} ms later, to rule out a second click.`) : null;
  if (isKnob(control)) {
    parts.push(
      section('Click', slot('action', '', { decorate: decorator(control) })),
      section('Double click', slot('double', ''), doubleHint),
      section('Turn', slot('left', 'Left'), slot('right', 'Right')),
      section('Turn while pressed', slot('press_left', 'Left'), slot('press_right', 'Right')),
    );
  } else {
    parts.push(
      section(b?.hold ? 'When tapped' : 'When pressed', slot('action', '', { decorate: decorator(control) })),
      section('When double clicked', slot('double', ''), doubleHint),
      section('When held', ...holdSection(control, b)),
    );
  }
  root.replaceChildren(...parts);
}

// ---------------------------------------------------------------- other views
function renameProfile(target, name) {
  if (!name || name === target.name) return;
  if (state.cfg.profile.some((p) => p.name === name)) return notify(`A profile named ${name} already exists.`, true);
  forEachAction(allPages(), (a) => {
    if (a.profile === target.name) a.profile = name;
    const t = lookTarget(a);
    if (t?.profile === target.name) t.profile = name;
  });
  target.name = name;
  save();
  draw();
}

function deleteProfile(index) {
  const target = state.cfg.profile[index];
  if (state.cfg.profile.length === 1) return notify('The deck needs at least one profile.', true);
  const uses = [];
  forEachAction(allPages(), (a, p) => { if (a.profile === target.name) uses.push(p.name); });
  if (uses.length) return notify(`Still used by: ${[...new Set(uses)].join(', ')}. Remove those first.`, true);
  confirmDialog(`Delete ${target.name}?`, 'All of its pages go with it.', 'Delete profile', () => {
    state.cfg.profile.splice(index, 1);
    state.profile = 0;
    state.page = 0;
    state.selected = null;
    save();
    draw();
  });
}

function profileDialog(index) {
  const target = state.cfg.profile[index];
  const name = h('input', { type: 'text', value: target.name, 'aria-label': 'Profile name', onchange: () => renameProfile(target, name.value.trim()) });
  const text = (key, label, hint) => {
    const input = h('input', {
      type: 'text', value: target[key] ?? '',
      oninput: () => {
        if (input.value) target[key] = input.value;
        else delete target[key];
        save();
        drawProfiles();
      },
    });
    return field(label, input, hint);
  };
  const flag = (key, label) => {
    const box = h('input', { type: 'checkbox', checked: target[key] ?? true, onchange: () => { target[key] = box.checked; save(); } });
    return h('label', { class: 'check' }, box, label);
  };
  openDialog(
    dialogHead('Profile settings'),
    h('div', { class: 'profile-form' },
      section('Name', name),
      section('Use this profile for', h('p', { class: 'hint' }, 'Every field you fill in has to match, and the first matching profile from the top wins. Leave them all empty for a profile that only opens through an action or as the default.'),
      text('exe', 'Program', 'For example zed.exe, or a full path.'),
      text('title', 'Window title contains'),
      text('title_regex', 'Window title matches regex')),
      section('When an app matches',
        flag('on_focus', 'Switch to it automatically while the app has focus'),
        flag('on_hold', 'Show its first page while holding a focused-app key'))),
    dialogFoot(h('button', { class: 'plain primary', onclick: () => $('dialog').close() }, 'Done')),
  );
  $('dialog').classList.add('medium');
}

const DATE_STYLES = [['', 'System format'], ['dd/MM/yyyy', '31/12/2026'], ['MM/dd/yyyy', '12/31/2026'], ['yyyy-MM-dd', '2026-12-31'], ['dd/MM', '31/12'], ['d MMM yyyy', '31 Dec 2026'], ['d MMM', '31 Dec']];

function dateField() {
  const cfg = state.cfg;
  const known = DATE_STYLES.some(([value]) => value === cfg.date_format);
  const custom = h('input', {
    type: 'text', value: cfg.date_format, placeholder: 'dd/MM/yyyy', hidden: known,
    oninput: () => { cfg.date_format = custom.value; save(); },
  });
  const style = h('select', {
    onchange: () => {
      const own = style.value === 'custom';
      custom.hidden = !own;
      if (!own) { cfg.date_format = style.value; save(); } else custom.focus();
    },
  }, ...DATE_STYLES.map(([value, text]) => h('option', { value, selected: value === cfg.date_format }, text)),
  h('option', { value: 'custom', selected: !known }, 'Custom'));
  return field('Date', h('div', { class: 'date-style' }, style, custom), 'Custom uses d, M and y, as in dd/MM/yy. Names follow the Windows language.');
}

function forgetButton() {
  const deck = state.devices.find((d) => d.id === state.device);
  if (!deck || deck.connected || !deck.id) return null;
  return h('button', {
    class: 'plain forget',
    onclick: () => confirmDialog(`Forget ${deck.label}?`, 'Its profiles and pages are deleted. If this deck is plugged in again, it starts from the shared config.', 'Forget deck', async () => {
      clearTimeout(saveTimer); // a pending save would bring the file straight back
      saveTimer = null;
      try {
        await invoke('forget_device', { device: deck.id });
      } catch (e) {
        return notify(`Not forgotten: ${e}`, true);
      }
      await loadDevices();
      state.view = 'keys';
      await switchDevice(firstDevice());
    }),
  }, 'Forget this deck');
}

const DAY_MS = 24 * 60 * 60 * 1000;

function drawUpdate() {
  const offer = state.update;
  const root = $('update');
  root.hidden = !offer;
  if (!offer) return root.replaceChildren();
  const notes = h('button', { class: 'link', onclick: () => invoke('open_release_notes') }, "What's new");
  if (offer.install === 'dev') {
    return root.replaceChildren(h('p', {}, `Version ${offer.version} is out`), notes);
  }
  const install = h('button', {
    class: 'plain primary',
    onclick: async () => {
      install.disabled = true;
      install.textContent = 'Updating';
      await flushSave();
      try {
        await invoke('install_update');
      } catch (e) {
        install.disabled = false;
        install.textContent = 'Update and restart';
        notify(`Update failed: ${e}`, true);
      }
    },
  }, 'Update and restart');
  root.replaceChildren(h('p', {}, `Version ${offer.version} is ready`), install, notes);
}

async function checkForUpdate(manual = false) {
  try {
    state.update = await invoke('check_update');
    drawUpdate();
    if (manual) notify(state.update ? `Version ${state.update.version} is available.` : 'You have the latest version.');
  } catch (e) {
    if (manual) notify(`Could not check for updates: ${e}`, true);
  }
}

function updateSettings() {
  const input = h('input', { type: 'checkbox', checked: state.cfg.check_updates, onchange: () => { state.cfg.check_updates = input.checked; save(); } });
  return section('Updates',
    h('p', { class: 'hint section-note' }, `Version ${state.version}`),
    h('label', { class: 'check' }, input, 'Check GitHub for new versions'),
    h('button', { class: 'plain', onclick: () => checkForUpdate(true) }, 'Check now'));
}

function obsStateText() {
  const obs = state.status?.obs;
  if (!obs) return 'Connects once a deck is plugged in.';
  return obs.connected ? 'Connected to OBS.' : obs.error || 'Connecting to OBS…';
}

function obsSettings() {
  const obs = state.cfg.obs;
  const text = (key, label, type, hint) => {
    const input = h('input', { type, value: obs[key], onchange: () => { obs[key] = input.value.trim(); save(); } });
    return field(label, input, hint);
  };
  const port = h('input', {
    type: 'number', min: 1, max: 65535, value: obs.port,
    onchange: () => { obs.port = Math.min(65535, Math.max(1, Number(port.value) || 4455)); port.value = obs.port; save(); },
  });
  return section('OBS Studio',
    h('p', { id: 'obs-state', class: 'hint section-note' }, obsStateText()),
    h('p', { class: 'hint' }, 'In OBS, open Tools, then WebSocket Server Settings, and enable the server. OBS actions appear in the list once it connects.'),
    text('host', 'Address', 'text'),
    field('Port', port),
    text('password', 'Server password', 'password', 'Kept as plain text in this deck\'s config file.'));
}

const pluginRows = () => (state.plugins.length
  ? state.plugins.map((p) => h('p', { class: 'hint' }, `${p.name}: ${p.error || (p.running ? 'Running.' : 'Starting…')}`))
  : [h('p', { class: 'hint' }, 'No plugins installed.')]);

function pluginSettings() {
  return section('Plugins',
    h('div', { id: 'plugin-list' }, ...pluginRows()),
    h('p', { class: 'hint' }, 'A plugin is a program that runs with your full permissions, so only add ones you trust. To add one, put its folder in the plugins folder, then quit from the tray and reopen OpenUlanzi.'),
    h('button', { class: 'plain', onclick: () => invoke('open_plugins_folder').catch((e) => notify(String(e), true)) }, 'Open plugins folder'));
}

function drawSettingsView() {
  const cfg = state.cfg;
  const number = (key, label, min, max, hint) => {
    const input = h('input', { type: 'number', min, max, value: cfg[key], onchange: () => { cfg[key] = Number(input.value); save(); } });
    return field(label, input, hint);
  };
  const toggle = (key, label) => {
    const input = h('input', { type: 'checkbox', checked: cfg[key], onchange: () => { cfg[key] = input.checked; save(); } });
    return h('label', { class: 'check' }, input, label);
  };
  const color = (key, label) => {
    const input = h('input', { type: 'color', value: cfg[key], onchange: () => { cfg[key] = input.value; save(); } });
    return h('label', { class: 'check' }, input, label);
  };
  const wide = h('select', { onchange: () => { cfg.wide = wide.value; save(); } },
    ...WIDE_DISPLAYS.filter(([id]) => id !== 'page_indicator').map(([value, text]) => h('option', { value, selected: value === cfg.wide }, text === 'Label and icon' ? 'Nothing' : text)));
  const weekday = h('select', { onchange: () => { cfg.weekday_format = weekday.value; save(); } },
    ...[['ddd', 'Short name'], ['dddd', 'Full name']].map(([value, text]) => h('option', { value, selected: value === cfg.weekday_format }, text)));
  const name = h('input', {
    type: 'text', value: cfg.name ?? '', placeholder: 'Named after its model',
    onchange: async () => { cfg.name = name.value.trim(); save(); await flushSave(); loadDevices(); },
  });
  const model = h('select', {
    onchange: async () => { cfg.model = model.value; save(); await flushSave(); loadDevices(); drawActions(); },
  }, ...[['d200x', 'D200X, with knobs and side buttons'], ['d200h', 'D200H, keys only'], ['d200', 'D200, keys only']]
    .map(([value, text]) => h('option', { value, selected: value === cfg.model }, text)));
  $('stage').replaceChildren(h('div', { class: 'sheet' },
    h('h1', {}, 'Settings'),
    h('p', {}, 'These apply to this deck. Start at login is in the tray menu.'),
    section('This deck',
      field('Deck name', name),
      field('Model', model, 'The deck cannot report its model, so this decides which controls are shown.'),
      forgetButton()),
    section('Brightness',
      number('brightness', 'While in use', 0, 100),
      number('locked_brightness', 'While the PC is locked', 0, 100)),
    section('Sleep',
      number('sleep_minutes', 'Turn the deck off after (minutes)', 0, 1440, 'With no keyboard or mouse input for this long. Any input wakes it, and so does pressing the deck. 0 keeps it on.'),
      toggle('sleep_with_display', 'Turn the deck off when Windows turns the screens off')),
    section('Timing',
      number('hold_ms', 'Hold to show a page (ms)', 50, 2000, 'For keys that also have a click action.'),
      number('hold_action_ms', 'Hold to run an action (ms)', 150, 5000, 'Keys can set their own time.'),
      number('double_ms', 'Double click gap (ms)', 100, 1000, 'Single clicks on keys with a double click action wait this long.')),
    section('Clock and wide screen',
      field('Wide screen on pages that do not set it', wide),
      toggle('twelve_hour', 'Use a 12-hour clock'),
      dateField(),
      field('Weekday', weekday)),
    section('Key defaults',
      color('color', 'Default icon colour'),
      color('background', 'Default key background')),
    obsSettings(),
    pluginSettings(),
    updateSettings()));
}

// ---------------------------------------------------------------- shell
function moveProfile(from, to) {
  const list = state.cfg.profile;
  if (to < 0 || to >= list.length || from === to) return;
  const current = list[state.profile];
  list.splice(to, 0, list.splice(from, 1)[0]);
  state.profile = list.indexOf(current);
  save();
  drawProfiles();
  document.querySelectorAll('.profile')[to]?.focus();
}

function profileHint(p, index) {
  const rule = p.exe || p.title || p.title_regex;
  if (index === defaultProfile()) return rule ? `Default, ${rule}` : 'Default';
  return rule || 'Opened by an action';
}

function addProfile() {
  let n = state.cfg.profile.length + 1;
  while (state.cfg.profile.some((p) => p.name === `Profile ${n}`)) n++;
  state.cfg.profile.push({ name: `Profile ${n}`, on_focus: true, on_hold: true, page: [{ name: 'Page 1', keys: {} }] });
  state.profile = state.cfg.profile.length - 1;
  state.page = 0;
  state.view = 'keys';
  state.selected = null;
  save();
  draw();
  profileDialog(state.profile);
}

function drawProfiles() {
  const rows = state.cfg.profile.map((p, i) => {
    const row = h('button', {
      class: 'profile',
      draggable: true,

      'aria-current': String(state.view === 'keys' && i === state.profile),
      onclick: () => { state.profile = i; state.page = 0; state.view = 'keys'; state.selected = null; draw(); },
      oncontextmenu: (e) => { e.preventDefault(); e.stopPropagation(); profileMenu(menuPoint(e), i); },
      onkeydown: (e) => {
        const step = e.altKey ? { ArrowUp: -1, ArrowDown: 1 }[e.key] : 0;
        if (step) { e.preventDefault(); moveProfile(i, i + step); }
      },
    }, h('span', { class: 'profile-name' }, p.name), h('small', {}, profileHint(p, i)));
    row.addEventListener('dragstart', (e) => e.dataTransfer.setData('text/plain', `profile:${i}`));
    row.addEventListener('dragover', (e) => { e.preventDefault(); row.classList.add('over'); });
    row.addEventListener('dragleave', () => row.classList.remove('over'));
    row.addEventListener('drop', (e) => {
      e.preventDefault();
      row.classList.remove('over');
      const [kind, from] = e.dataTransfer.getData('text/plain').split(':');
      if (kind === 'profile') moveProfile(Number(from), i);
    });
    return row;
  });
  $('profiles').replaceChildren(h('div', { class: 'section-head' }, h('h2', {}, 'Profiles'), iconButton('plus-bold', 'Add profile', addProfile)), ...rows);
}

function drawViews() {
  for (const b of document.querySelectorAll('.view')) {
    b.setAttribute('aria-current', String(b.dataset.view === state.view));
  }
}

function draw() {
  drawViews();
  drawProfiles();
  ({ keys: drawKeysView, settings: drawSettingsView })[state.view]();
  document.body.classList.toggle('no-panels', state.view !== 'keys');
  drawInspector();
}

function drawStatus(status) {
  state.status = status;
  const obs = JSON.stringify(status?.obs ?? null);
  if (obs !== state.obs && state.cfg) {
    state.obs = obs;
    drawActions();
    if ($('obs-state')) $('obs-state').textContent = obsStateText();
  }
  if (status && state.cfg && JSON.stringify(status.plugins) !== JSON.stringify(state.plugins)) {
    usePlugins(status.plugins);
    drawActions();
    $('plugin-list')?.replaceChildren(...pluginRows());
  }
  const connection = $('connection');
  if (!status?.connected) {
    $('live-deck').replaceChildren();
    connection.className = 'connection';
    connection.textContent = 'Deck not connected';
    $('showing').className = 'showing';
    $('showing').textContent = 'Plug it in and close Ulanzi Studio.';
  } else {
    connection.className = 'connection on';
    connection.textContent = status.asleep ? 'Deck asleep' : status.locked ? 'Deck locked' : 'Deck connected';
    const held = status.layers.length > 0;
    $('showing').className = held ? 'showing held' : 'showing';
    $('showing').textContent = held ? `Holding ${status.layers.at(-1)}` : `Showing ${status.profile}, ${status.page}`;
  }
  if (status) drawMiniDeck($('live-deck'), status);
}

async function start() {
  for (const b of document.querySelectorAll('.view')) {
    b.addEventListener('click', () => { state.view = b.dataset.view; draw(); });
    b.prepend(h('span', { 'data-glyph': 'gear-six-bold' }));
  }
  document.addEventListener('contextmenu', (e) => {
    if (!e.target.closest('input, textarea')) e.preventDefault();
    closeMenu();
  });
  document.addEventListener('pointerdown', (e) => { if (!e.target.closest('.menu')) closeMenu(); });
  document.addEventListener('keydown', (e) => {
    const typing = e.target.closest('input, textarea, select, [contenteditable]');
    if ((e.ctrlKey || e.metaKey) && !e.altKey && !typing && !$('dialog').open && state.view === 'keys' && state.selection.size) {
      const key = e.key.toLowerCase();
      if (key === 'c') { e.preventDefault(); copyControls(selectedList()); }
      if (key === 'v' && state.clipboard) { e.preventDefault(); pasteControls(selectedList()); }
      return;
    }
    if (e.key !== 'Escape') return;
    if (document.getElementById('menu')) closeMenu(true);
    else if (!$('dialog').open && state.selection.size) select(null);
  });
  state.icons = new Map(await invoke('icon_library'));
  for (const slot of document.querySelectorAll('[data-glyph]')) slot.replaceWith(icon(slot.dataset.glyph));
  state.collapsed = new Set(JSON.parse(recall('collapsed') ?? '[]'));
  const { listen } = window.__TAURI__.event;
  $('device').addEventListener('change', () => switchDevice($('device').value));
  state.version = await invoke('app_version');
  usePlugins(await invoke('get_plugins'));
  await loadDevices();
  await switchDevice(firstDevice());
  if (state.cfg?.check_updates) checkForUpdate();
  setInterval(() => { if (state.cfg?.check_updates) checkForUpdate(); }, DAY_MS);
  listen('deck-status', ({ payload }) => {
    const known = state.devices.find((d) => d.id === payload.device);
    if (known && known.connected !== payload.connected) {
      known.connected = payload.connected;
      drawDevices();
    }
    if (payload.device === state.device) drawStatus(payload);
  });
  listen('devices-changed', async () => {
    await loadDevices();
    // The starting config only stands in until a real deck shows up.
    if (state.device === '') await switchDevice(firstDevice());
  });
}

function firstDevice() {
  return (state.devices.find((d) => d.connected) ?? state.devices[0]).id;
}

function drawDevices() {
  const select = $('device');
  select.hidden = state.devices.length < 2;
  select.replaceChildren(...state.devices.map((d) => h('option', { value: d.id, selected: d.id === state.device },
    d.connected ? d.label : `${d.label} (not connected)`)));
}

async function loadDevices() {
  state.devices = await invoke('list_devices');
  drawDevices();
}

async function switchDevice(id) {
  await flushSave(); // pending edits belong to the deck being left
  state.device = id;
  drawDevices();
  try {
    state.cfg = await invoke('get_config', { device: id });
  } catch (e) {
    $('stage').replaceChildren(h('div', { class: 'sheet' }, h('h1', {}, 'The config file has a problem'), h('p', {}, String(e)), h('p', {}, 'Fix the file, then reopen OpenUlanzi.')));
    return;
  }
  state.profile = 0;
  state.page = 0;
  state.selected = null;
  drawActions();
  draw();
  drawStatus(await invoke('get_status', { device: id }));
}

start();
