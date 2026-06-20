import { invoke } from "@tauri-apps/api/core";
import { listen, emit } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";

// Types matching Rust backend (AppConfig serialized JSON keys)
interface AppConfig {
  // Paths (snake_case in backend)
  audio_dir: string;
  text_file_path: string;
  names_file_path: string;
  screenshot_dir: string;
  // OCR settings (UPPER_CASE in backend serde)
  RESOLUTION_DOWNSCALE: number;
  CAPTURE_INTERVAL: number;
  MIN_HEIGHT: number;
  MAX_HEIGHT: number;
  OCR_MIN_CONFIDENCE: number;
  CAPTURE_MODE: string;
  CAPTURE_WINDOW_QUERY: string;
  CAPTURE_MONITOR?: string;
  ENABLE_REMOVE_CHARACTER_NAME: boolean;
  ENABLE_SCREENSHOTS: boolean;
  ENABLE_PARAGRAPH_OCR: boolean;
  ENABLE_TYPEWRITER_WAIT: boolean;
  ENABLE_REGION_OVERLAY: boolean;
  ENABLE_OUTLINE_TEXT_MODE?: boolean;
  OUTLINE_WHITE_THRESHOLD?: number;
  OUTLINE_DARK_THRESHOLD?: number;
  USE_CENTER_LINE_1: boolean;
  USE_CENTER_LINE_2: boolean;
  USE_CENTER_LINE_3: boolean;
  CENTER_LINE_2_START: number;
  CENTER_LINE_3_START_RATIO: number;
  CENTER_LINE_MARGIN: number;
  // Audio settings
  VOLUME_REDUCTION_LEVEL: number;
  READER_VOLUME: number;
  ENABLE_OUTPUT2_SYSTEM: boolean;
  ENABLE_DYNAMIC_SPEED: boolean;
  BASE_PLAYBACK_SPEED: number;
  OVERLAP_PLAYBACK_SPEED: number;
  AUDIO_QUEUE_SIZE: number;
  // UI Behavior
  MINIMIZE_TO_TRAY_ON_READER_START?: boolean;
  // Monitor / capture
  resolution: string;
  monitor: MonitorRect;
  monitor2_enabled: boolean;
  monitor2_top: number;
  monitor2_left: number;
  monitor2_width: number;
  monitor2_height: number;
  // Hotkeys
  key_bindings: KeyBindings;
}

interface MonitorRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

type KeyBindings = Record<string, string>;

interface RecentPreset {
  path: string;
  name: string;
  timestamp?: number;
}

// Global state
let currentConfig: AppConfig | null = null;
let readerEnabled = false;
let editingHotkey: string | null = null;
// Suppress config auto-save while we programmatically populate the UI
// (loadConfigIntoUI dispatches synthetic 'input' events to refresh displays).
let suppressConfigSave = false;
// Becomes true only after the config has been successfully loaded into the UI.
// Until then we must NEVER send update_config, or we'd overwrite the backend
// config (and audio_prefs) with empty/default HTML values.
let configLoaded = false;

// Hotkey action labels (match backend action names in constants.rs)
const hotkeyLabels: Record<string, string> = {
  toggle_reader: "Włącz/wyłącz lektor",
  interrupt_audio: "Przerwij audio",
  skip_next_line: "Pomiń do następnej linii (+10% speed)",
  switch_monitor_toggle: "Przełącz region",
  toggle_areas: "Pokaż/ukryj obszar OCR",
  open_settings: "Pokaż/ukryj ustawienia",
  volume_down: "Zmniejsz głośność",
  volume_up: "Zwiększ głośność",
  base_speed_down: "Zmniejsz prędkość bazową",
  base_speed_up: "Zwiększ prędkość bazową",
  overlap_speed_down: "Zmniejsz prędkość doganiania",
  overlap_speed_up: "Zwiększ prędkość doganiania",
  test_sound: "Test dźwięku",
  debug_console: "Konsola debug"
};

const hotkeyGroups = [
  {
    title: "System",
    actions: ["toggle_reader", "toggle_areas", "open_settings", "debug_console"]
  },
  {
    title: "Audio",
    actions: ["interrupt_audio", "volume_up", "volume_down", "test_sound"]
  },
  {
    title: "Prędkość",
    actions: ["base_speed_up", "base_speed_down", "overlap_speed_up", "overlap_speed_down"]
  }
];

// Toast notifications
function showToast(message: string, type: 'success' | 'error' | 'warning' | 'info' = 'info') {
  const container = document.getElementById('toast-container');
  if (!container) return;

  // Play the notification sound (fire-and-forget).
  invoke('play_sound', { name: 'announcement' }).catch(() => {});

  const toast = document.createElement('div');
  toast.className = `toast${type === 'error' ? ' error' : ''}`;
  
  const iconHtml = type === 'error'
    ? '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>'
    : '';

  toast.innerHTML = `
    <span class="toast-icon">${iconHtml}</span>
    <span class="toast-message">${message}</span>
  `;

  container.appendChild(toast);
  
  setTimeout(() => {
    toast.style.opacity = '0';
    setTimeout(() => toast.remove(), 300);
  }, 3000);
}

// Make the WebView feel like a native app rather than a browser:
// - no default right-click menu (Back / Refresh / Save as / Print / Inspect)
// - no WebView2 autofill ("Zapisane informacje") suggestions on path fields
function hardenWebview() {
  // Block the default WebView2 context menu.
  document.addEventListener('contextmenu', (e) => e.preventDefault());

  // Disable autofill/autocomplete/spellcheck on every input.
  document.querySelectorAll('input').forEach((el) => {
    el.setAttribute('autocomplete', 'off');
    el.setAttribute('autocorrect', 'off');
    el.setAttribute('autocapitalize', 'off');
    el.setAttribute('spellcheck', 'false');
  });
}

// Light/dark theme toggle. The initial theme is applied by an inline script
// in index.html (before paint); here we just wire the toggle button and keep
// the button label/icon in sync, persisting the choice in localStorage.
function initializeTheme() {
  const toggle = document.getElementById('theme-toggle');
  const icon = document.getElementById('theme-toggle-icon');
  const label = document.getElementById('theme-toggle-label');

  const apply = (theme: string) => {
    document.documentElement.setAttribute('data-theme', theme);
    try {
      localStorage.setItem('gr-theme', theme);
    } catch (e) {
      // localStorage may be unavailable; theme still applies for this session.
    }
    // Button shows the action it will perform (i.e. the other theme).
    if (icon) icon.innerHTML = theme === 'dark' 
      ? '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"/><line x1="12" y1="1" x2="12" y2="3"/><line x1="12" y1="21" x2="12" y2="23"/><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"/><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"/><line x1="1" y1="12" x2="3" y2="12"/><line x1="21" y1="12" x2="23" y2="12"/><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"/><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"/></svg>'
      : '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>';
    if (label) label.textContent = theme === 'dark' ? 'Jasny' : 'Ciemny';
    // Broadcast to the other windows (debug, launcher) so they switch live too.
    emit('theme_changed', theme).catch(() => {});
  };

  // Sync UI with whatever the inline script already set.
  const current = document.documentElement.getAttribute('data-theme') || 'light';
  apply(current);

  toggle?.addEventListener('click', () => {
    const now = document.documentElement.getAttribute('data-theme') === 'dark' ? 'dark' : 'light';
    apply(now === 'dark' ? 'light' : 'dark');
  });
}

// Detect the game window resolution via the backend, using the window query
// from the "Szybki start" tab. Returns [width, height] or null.
// `silent` suppresses toasts (used for automatic runs).
async function detectWindowResolution(silent = false): Promise<[number, number] | null> {
  const query = (document.getElementById('capture-window-query') as HTMLInputElement).value;
  if (!query.trim()) {
    if (!silent) showToast('Najpierw wskaż okno gry w „Szybki start"', 'warning');
    return null;
  }
  try {
    return await invoke<[number, number] | null>('detect_window_resolution', { query });
  } catch (e) {
    console.error('detect_window_resolution failed:', e);
    if (!silent) showToast('Nie udało się wykryć rozdzielczości okna', 'error');
    return null;
  }
}

// Updates the resolution info in the "Okno gry" card
async function updateWindowResolutionInfo() {
  const info = document.getElementById('window-resolution-info');
  if (!info) return;
  const res = await detectWindowResolution(true);
  info.textContent = res
    ? `Rozdzielczość: ${res[0]}×${res[1]}`
    : 'Rozdzielczość: --';
}

// Show which exact window the reader is set to read (the picked window title).
function setActiveWindowInfo(title: string | null) {
  const el = document.getElementById('active-window-info');
  if (el) el.textContent = title ? `Czyta z okna: ${title}` : '';
  const windowCard = document.getElementById('window-card-value');
  if (windowCard) windowCard.textContent = title || '--';
  updateWindowResolutionInfo();
}

// Wire up the "Zaznacz obszar na ekranie" (Win+Shift+S style) buttons.
// The backend hides the main window, shows a snipping overlay, and returns the
// chosen rectangle already converted to base-resolution region coordinates.
function setupRegionSelector() {
  wireRegionButton('select-region-btn', 1);
  wireRegionButton('select-region2-btn', 2);
  wireManualToggle('toggle-manual-region1', 'manual-region1');
  wireManualToggle('toggle-manual-region2', 'manual-region2');
}

function wireManualToggle(btnId: string, contentId: string) {
  const btn = document.getElementById(btnId) as HTMLButtonElement | null;
  const content = document.getElementById(contentId) as HTMLElement | null;
  if (!btn || !content) return;
  btn.addEventListener('click', () => {
    content.style.display = content.style.display === 'none' ? 'block' : 'none';
  });
}

function wireRegionButton(btnId: string, region: 1 | 2) {
  const btn = document.getElementById(btnId) as HTMLButtonElement | null;
  if (!btn) return;

  btn.addEventListener('click', async () => {
    const original = btn.textContent;
    btn.disabled = true;
    try {
      // A target game window is REQUIRED: the selection only happens over that
      // window. If no exe/window is set, make the user pick one first.
      const queryEl = document.getElementById('capture-window-query') as HTMLInputElement;
      if (!queryEl.value.trim()) {
        btn.textContent = 'Wybierz okno gry…';
        const picked = await invoke<{ query: string; title: string } | null>('pick_window');
        if (!picked) {
          showToast('Najpierw wybierz okno gry', 'warning');
          return;
        }
        queryEl.value = picked.query;
        setActiveWindowInfo(picked.title);
        await updateConfig({ CAPTURE_WINDOW_QUERY: picked.query });
        updateWindowResolutionInfo();
      }

      btn.textContent = 'Zaznaczanie… (przeciągnij myszką, ESC anuluje)';
      const r = await invoke<{ left: number; top: number; width: number; height: number } | null>(
        'select_screen_region'
      );
      if (!r) {
        showToast('Zaznaczanie anulowane', 'warning');
        return;
      }

      const p = region === 1 ? 'monitor' : 'monitor2';
      (document.getElementById(`${p}-top`) as HTMLInputElement).value = r.top.toString();
      (document.getElementById(`${p}-left`) as HTMLInputElement).value = r.left.toString();
      (document.getElementById(`${p}-width`) as HTMLInputElement).value = r.width.toString();
      (document.getElementById(`${p}-height`) as HTMLInputElement).value = r.height.toString();

      // Persist the new region so capture + overlay pick it up immediately.
      if (region === 1) {
        await updateConfig({ monitor: { top: r.top, left: r.left, width: r.width, height: r.height } });
      } else {
        await updateConfig({
          monitor2_top: r.top,
          monitor2_left: r.left,
          monitor2_width: r.width,
          monitor2_height: r.height,
        });
      }
      showToast(`Region ${region} ustawiony: ${r.width}×${r.height} (${r.left}, ${r.top})`, 'success');
    } catch (error) {
      console.error('select_screen_region failed:', error);
      showToast(typeof error === 'string' ? error : 'Nie udało się zaznaczyć obszaru', 'error');
    } finally {
      btn.disabled = false;
      btn.textContent = original;
    }
  });
}

// Populate the monitor picker (shown only when more than one display exists).
async function setupMonitorPicker() {
  const sel = document.getElementById('capture-monitor') as HTMLSelectElement | null;
  const card = document.getElementById('monitor-picker-card');
  if (!sel || !card) return;

  let monitors: Array<{ id: string; name: string; width: number; height: number; primary: boolean }> = [];
  try {
    monitors = await invoke('list_monitors');
  } catch (e) {
    console.error('list_monitors failed:', e);
  }

  // Reset to just the "auto" option, then add one per monitor.
  sel.innerHTML = '<option value="">Automatycznie (podążaj za oknem)</option>';
  for (const m of monitors) {
    const label = m.id.replace(/^\\\\\.\\/, '');
    const opt = document.createElement('option');
    opt.value = m.id;
    opt.textContent = `${label} — ${m.width}×${m.height}${m.primary ? ' (główny)' : ''}`;
    sel.appendChild(opt);
  }

  // Only worth showing with multiple monitors.
  card.style.display = monitors.length > 1 ? '' : 'none';

  sel.addEventListener('change', async () => {
    try {
      await updateConfig({ CAPTURE_MONITOR: sel.value });
    } catch (e) {
      console.error('Failed to save monitor selection:', e);
    }
  });
}

// Tab switching
function initializeTabs() {
  const tabItems = document.querySelectorAll('.nav-item');
  const tabContents = document.querySelectorAll('.tab-content');
  
  tabItems.forEach(item => {
    item.addEventListener('click', () => {
      const tabName = item.getAttribute('data-tab');
      
      // Update active states
      tabItems.forEach(t => t.classList.remove('active'));
      tabContents.forEach(c => c.classList.remove('active'));
      
      item.classList.add('active');
      document.querySelector(`[data-tab-content="${tabName}"]`)?.classList.add('active');
    });
  });
}

// Slider value updates
function initializeSliders() {
  const sliders = document.querySelectorAll('input[type="range"]');
  
  sliders.forEach(slider => {
    const valueSpan = slider.parentElement?.querySelector('.slider-value');
    if (!valueSpan) return;
    
    const updateValue = () => {
      const value = parseFloat((slider as HTMLInputElement).value);
      const id = slider.id;
      
      if (id.includes('speed')) {
        valueSpan.textContent = `${value.toFixed(2)}x`;
      } else if (id.includes('ratio') || id.includes('coverage')) {
        valueSpan.textContent = value.toFixed(2);
      } else if (id.includes('interval')) {
        valueSpan.textContent = `${value.toFixed(1)}s`;
      } else if (id.includes('volume') || id.includes('threshold')) {
        valueSpan.textContent = `${Math.round(value)}%`;
      } else {
        valueSpan.textContent = value.toString();
      }
    };
    
    slider.addEventListener('input', updateValue);
    updateValue(); // Initialize
  });
}

// --- Dual-handle playback speed slider (base + overlap on one track) ---
const SPEED_MIN = 0.5;
const SPEED_MAX = 2.0;

// Recompute the coloured fill and the two value labels from the input values.
function refreshSpeedSlider() {
  const base = document.getElementById('playback-speed') as HTMLInputElement | null;
  const overlap = document.getElementById('overlap-speed') as HTMLInputElement | null;
  const fill = document.getElementById('speed-fill') as HTMLElement | null;
  const baseVal = document.getElementById('base-speed-val');
  const overlapVal = document.getElementById('overlap-speed-val');
  if (!base || !overlap) return;

  const b = parseFloat(base.value);
  const o = parseFloat(overlap.value);
  const span = SPEED_MAX - SPEED_MIN;
  const bp = ((b - SPEED_MIN) / span) * 100;
  const op = ((o - SPEED_MIN) / span) * 100;
  if (fill) {
    fill.style.left = `${Math.min(bp, op)}%`;
    fill.style.width = `${Math.abs(op - bp)}%`;
  }
  if (baseVal) baseVal.textContent = `${b.toFixed(2)}x`;
  if (overlapVal) overlapVal.textContent = `${o.toFixed(2)}x`;
}

// Wire the two range handles, keeping overlap >= base * 1.05.
function setupSpeedSlider() {
  const base = document.getElementById('playback-speed') as HTMLInputElement | null;
  const overlap = document.getElementById('overlap-speed') as HTMLInputElement | null;
  if (!base || !overlap) return;

  const enforce = (changed: 'base' | 'overlap') => {
    let b = parseFloat(base.value);
    let o = parseFloat(overlap.value);
    // Base can't go so high that base*1.05 exceeds the max.
    const baseMax = Math.floor((SPEED_MAX / 1.05) * 20) / 20; // round to 0.05
    if (b > baseMax) {
      b = baseMax;
      base.value = b.toFixed(2);
    }
    const minOverlap = Math.round(b * 1.05 * 100) / 100;
    if (changed === 'base') {
      if (o < minOverlap) o = minOverlap;
    } else {
      if (o < minOverlap) o = minOverlap; // dragging overlap below the floor
    }
    overlap.value = o.toFixed(2);
    refreshSpeedSlider();
  };

  base.addEventListener('input', () => enforce('base'));
  overlap.addEventListener('input', () => enforce('overlap'));

  // Both range inputs overlap on one track; when the two thumbs are close the
  // top one would block the other. On pointer-down, give priority (z-index) to
  // whichever thumb is closer to the click, so both are always grabbable.
  const container = base.closest('.dual-slider') as HTMLElement | null;
  container?.addEventListener('pointerdown', (e) => {
    const rect = container.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    const val = SPEED_MIN + ratio * (SPEED_MAX - SPEED_MIN);
    const baseCloser = Math.abs(val - parseFloat(base.value)) <= Math.abs(val - parseFloat(overlap.value));
    base.style.zIndex = baseCloser ? '5' : '3';
    overlap.style.zIndex = baseCloser ? '3' : '5';
  });

  enforce('base');
}

// File/folder pickers
function initializePickers() {
  const pickerButtons = document.querySelectorAll('[data-picker]');
  
  pickerButtons.forEach(button => {
    button.addEventListener('click', async () => {
      const targetId = button.getAttribute('data-picker');
      if (!targetId) return;
      
      const input = document.getElementById(targetId) as HTMLInputElement;
      if (!input) return;
      
      try {
        const isFolder = targetId === 'audio-dir' || targetId === 'screenshot-dir';
        // Dialogi i imiona postaci to pliki tekstowe — pokazuj tylko .txt.
        const isTxt = targetId === 'text-file' || targetId === 'names-file';

        const selected = await open({
          directory: isFolder,
          multiple: false,
          title: `Wybierz ${isFolder ? 'folder' : 'plik'}`,
          filters: isTxt ? [{ name: 'Plik tekstowy', extensions: ['txt'] }] : undefined,
        });
        
        if (selected && typeof selected === 'string') {
          input.value = selected;
          // Dispatch 'input' so the debounced config listener fires (text inputs listen for 'input')
          input.dispatchEvent(new Event('input', { bubbles: true }));
          input.dispatchEvent(new Event('change', { bubbles: true }));
        }
      } catch (error) {
        console.error('Picker error:', error);
        showToast('Błąd wyboru pliku/folderu', 'error');
      }
    });
  });
}

// Load config into UI
function loadConfigIntoUI(config: AppConfig) {
  currentConfig = config;
  suppressConfigSave = true;
  try {
    loadConfigIntoUIInner(config);
    configLoaded = true;
  } finally {
    suppressConfigSave = false;
  }
}

function loadConfigIntoUIInner(config: AppConfig) {
  
  // Files tab
  (document.getElementById('audio-dir') as HTMLInputElement).value = config.audio_dir ?? '';
  (document.getElementById('text-file') as HTMLInputElement).value = config.text_file_path ?? '';
  (document.getElementById('names-file') as HTMLInputElement).value = config.names_file_path ?? '';
  (document.getElementById('screenshot-dir') as HTMLInputElement).value = config.screenshot_dir ?? '';
  
  // Audio tab (slider is 0-100%, config stores 0.0-1.0)
  {
    const volEl = document.getElementById('volume-reduction') as HTMLInputElement;
    volEl.value = Math.round((config.VOLUME_REDUCTION_LEVEL ?? 0) * 100).toString();
    const span = volEl.parentElement?.querySelector('.slider-value');
    if (span) span.textContent = `${volEl.value}%`;
  }
  {
    const rvEl = document.getElementById('reader-volume') as HTMLInputElement | null;
    if (rvEl) {
      rvEl.value = Math.round((config.READER_VOLUME ?? 1) * 100).toString();
      const span = rvEl.parentElement?.querySelector('.slider-value');
      if (span) span.textContent = `${rvEl.value}%`;
    }
  }
  (document.getElementById('playback-speed') as HTMLInputElement).value = config.BASE_PLAYBACK_SPEED.toString();
  const overlapEl = document.getElementById('overlap-speed') as HTMLInputElement | null;
  if (overlapEl) overlapEl.value = config.OVERLAP_PLAYBACK_SPEED.toString();
  refreshSpeedSlider();
  document.querySelectorAll<HTMLInputElement>('input[name="queue-size"]').forEach(radio => {
    radio.checked = parseInt(radio.value) === config.AUDIO_QUEUE_SIZE;
  });
  (document.getElementById('minimize-to-tray') as HTMLInputElement).checked = config.MINIMIZE_TO_TRAY_ON_READER_START ?? false;
  
  // OCR tab
  (document.getElementById('capture-interval') as HTMLInputElement).value = config.CAPTURE_INTERVAL.toString();
  // capture mode is always "window" (mode selector removed from UI)
  (document.getElementById('capture-window-query') as HTMLInputElement).value = config.CAPTURE_WINDOW_QUERY || '';
  const monitorSel = document.getElementById('capture-monitor') as HTMLSelectElement | null;
  if (monitorSel) monitorSel.value = config.CAPTURE_MONITOR || '';
  (document.getElementById('min-height') as HTMLInputElement).value = config.MIN_HEIGHT.toString();
  (document.getElementById('max-height') as HTMLInputElement).value = config.MAX_HEIGHT.toString();
  (document.getElementById('enable-remove-character-name') as HTMLInputElement).checked = config.ENABLE_REMOVE_CHARACTER_NAME;
  (document.getElementById('enable-screenshots') as HTMLInputElement).checked = config.ENABLE_SCREENSHOTS;
  (document.getElementById('enable-paragraph-ocr') as HTMLInputElement).checked = config.ENABLE_PARAGRAPH_OCR;
  (document.getElementById('enable-typewriter-wait') as HTMLInputElement).checked = config.ENABLE_TYPEWRITER_WAIT;
  (document.getElementById('enable-region-overlay') as HTMLInputElement).checked = config.ENABLE_REGION_OVERLAY ?? false;
  const outlineModeEl = document.getElementById('enable-outline-text-mode') as HTMLInputElement | null;
  if (outlineModeEl) outlineModeEl.checked = config.ENABLE_OUTLINE_TEXT_MODE ?? false;
  (document.getElementById('use-center-line-1') as HTMLInputElement).checked = config.USE_CENTER_LINE_1;
  (document.getElementById('use-center-line-2') as HTMLInputElement).checked = config.USE_CENTER_LINE_2;
  (document.getElementById('use-center-line-3') as HTMLInputElement).checked = config.USE_CENTER_LINE_3;
  (document.getElementById('center-line-2-start') as HTMLInputElement).value = config.CENTER_LINE_2_START.toString();
  (document.getElementById('center-line-3-ratio') as HTMLInputElement).value = config.CENTER_LINE_3_START_RATIO.toString();
  (document.getElementById('center-line-margin') as HTMLInputElement).value = config.CENTER_LINE_MARGIN.toString();
  
  // Advanced tab
  // (Region base resolution comes from the preset JSON `resolution` field;
  // there is no UI control for it. Region coords scale from that base to the
  // live window, so 4K/1080p/ultrawide presets all work.)
  (document.getElementById('monitor-top') as HTMLInputElement).value = config.monitor.top.toString();
  (document.getElementById('monitor-left') as HTMLInputElement).value = config.monitor.left.toString();
  (document.getElementById('monitor-width') as HTMLInputElement).value = config.monitor.width.toString();
  (document.getElementById('monitor-height') as HTMLInputElement).value = config.monitor.height.toString();
  (document.getElementById('monitor2-enabled') as HTMLInputElement).checked = config.monitor2_enabled;
  (document.getElementById('monitor2-top') as HTMLInputElement).value = config.monitor2_top.toString();
  (document.getElementById('monitor2-left') as HTMLInputElement).value = config.monitor2_left.toString();
  (document.getElementById('monitor2-width') as HTMLInputElement).value = config.monitor2_width.toString();
  (document.getElementById('monitor2-height') as HTMLInputElement).value = config.monitor2_height.toString();
  
  // Update all slider displays
  document.querySelectorAll<HTMLInputElement>('input[type="range"]').forEach(slider => {
    slider.dispatchEvent(new Event('input'));
  });
  
  // Render hotkeys
  renderHotkeys(config.key_bindings);
}

// Render hotkeys grid
function renderHotkeys(bindings: KeyBindings) {
  const grid = document.getElementById('hotkeys-grid');
  if (!grid) return;
  
  if (grid.childElementCount > 0) {
    grid.querySelectorAll('.hotkey-value').forEach(el => {
      const action = el.getAttribute('data-action');
      if (action) el.textContent = bindings[action] || '(nie ustawiono)';
    });
    return;
  }

  hotkeyGroups.forEach(group => {
    const panel = document.createElement('div');
    panel.className = 'panel hotkey-panel';
    
    const panelHeader = document.createElement('div');
    panelHeader.className = 'panel-header';
    const headerTitle = document.createElement('h3');
    headerTitle.textContent = group.title;
    panelHeader.appendChild(headerTitle);
    panel.appendChild(panelHeader);
    
    const panelContent = document.createElement('div');
    panelContent.className = 'hotkey-content';
    
    group.actions.forEach(action => {
      const key = bindings[action];
      const row = document.createElement('div');
      row.className = 'hotkey-row';
      
      const label = document.createElement('div');
      label.className = 'hotkey-label';
      label.textContent = hotkeyLabels[action] ?? action;
      
      const value = document.createElement('div');
      value.className = 'hotkey-value';
      value.textContent = key || '(nie ustawiono)';
      value.dataset.action = action;
      
      value.addEventListener('click', () => {
        if (editingHotkey) {
          document.querySelectorAll('.hotkey-value.editing').forEach(el => {
            el.classList.remove('editing');
            const prevAction = el.getAttribute('data-action');
            if (prevAction && currentConfig) {
              el.textContent = currentConfig.key_bindings[prevAction] || '(nie ustawiono)';
            }
          });
        }
        
        editingHotkey = action;
        value.classList.add('editing');
        value.textContent = 'Naciśnij klawisz...';
      });
      
      row.appendChild(label);
      row.appendChild(value);
      panelContent.appendChild(row);
    });
    
    panel.appendChild(panelContent);
    grid.appendChild(panel);
  });
}

// Map a captured KeyboardEvent.key to the backend's expected key name
// (must match constants::ALLOWED_KEYS / hotkeys::key_name_to_vk). Returns null
// for keys the backend doesn't support.
function normalizeCapturedKey(e: KeyboardEvent): string | null {
  const special: Record<string, string> = {
    'Home': 'home', 'End': 'end', 'Insert': 'insert', 'Delete': 'delete',
    'PageUp': 'page_up', 'PageDown': 'page_down',
    'Tab': 'tab', 'Backspace': 'backspace', ' ': 'space', 'Spacebar': 'space',
    'Escape': 'esc', 'Esc': 'esc',
    '`': '`', '~': '`', '_': '_', "'": "'", '"': "'",
  };
  if (special[e.key]) return special[e.key];
  // Function keys F1–F12
  if (/^F([1-9]|1[0-2])$/.test(e.key)) return e.key.toLowerCase();
  // Single alphanumeric character
  const lower = e.key.toLowerCase();
  if (/^[a-z0-9]$/.test(lower)) return lower;
  return null;
}

// Unified keydown handler: hotkey capture (when editing) + in-app shortcuts
function setupKeyboardHandler() {
  document.addEventListener('keydown', (e) => {
    if (editingHotkey) {
      handleHotkeyCapture(e);
      return;
    }
    handleInAppShortcut(e);
  });
}

function handleInAppShortcut(e: KeyboardEvent) {
  if (!currentConfig) return;

  const ae = document.activeElement as HTMLElement | null;
  if (ae) {
    const tag = ae.tagName.toLowerCase();
    if (tag === 'input' || tag === 'textarea' || tag === 'select' || ae.isContentEditable) {
      return;
    }
  }

  const backendKey = normalizeCapturedKey(e);
  if (!backendKey) return;

  const modifiers: string[] = [];
  if (e.ctrlKey) modifiers.push('ctrl');
  if (e.shiftKey) modifiers.push('shift');
  if (e.altKey) modifiers.push('alt');
  const hotkeyString = [...modifiers, backendKey].join('+');

  const bindings = currentConfig.key_bindings as Record<string, string>;
  const match = Object.entries(bindings).find(
    ([, val]) => val && val.toLowerCase() === hotkeyString.toLowerCase()
  );
  if (!match) return;

  e.preventDefault();
  invoke('trigger_hotkey', { action: match[0] }).catch((err) => {
    console.error('trigger_hotkey failed:', err);
  });
}

function handleHotkeyCapture(e: KeyboardEvent) {
  e.preventDefault();
  e.stopPropagation();

  if (e.key === 'Escape') {
    const cancelEl = document.querySelector(`.hotkey-value[data-action="${editingHotkey}"]`);
    if (cancelEl && currentConfig) {
      cancelEl.textContent = (currentConfig.key_bindings as Record<string, string>)[editingHotkey!] || '(nie ustawiono)';
      cancelEl.classList.remove('editing');
    }
    editingHotkey = null;
    return;
  }
  
  const modifiers: string[] = [];
  if (e.ctrlKey) modifiers.push('ctrl');
  if (e.shiftKey) modifiers.push('shift');
  if (e.altKey) modifiers.push('alt');
  
  const key = e.key.toLowerCase();
  
  if (['control', 'shift', 'alt', 'meta'].includes(key)) return;

  const backendKey = normalizeCapturedKey(e);
  if (!backendKey) {
    showToast('Ten klawisz nie jest obsługiwany. Użyj liter, cyfr, F1–F12 lub Home/End/Insert/Delete/PageUp/PageDown.', 'warning');
    const cancelEl = document.querySelector(`.hotkey-value[data-action="${editingHotkey}"]`);
    if (cancelEl && currentConfig) {
      cancelEl.textContent = (currentConfig.key_bindings as Record<string, string>)[editingHotkey!] || '(nie ustawiono)';
      cancelEl.classList.remove('editing');
    }
    editingHotkey = null;
    return;
  }
  
  const hotkeyString = [...modifiers, backendKey].join('+');

  if (currentConfig) {
    const bindings = currentConfig.key_bindings as Record<string, string>;
    const clash = Object.entries(bindings).find(
      ([act, val]) => act !== editingHotkey && val && val.toLowerCase() === hotkeyString.toLowerCase()
    );
    if (clash) {
      const clashLabel = hotkeyLabels[clash[0]] ?? clash[0];
      showToast(`Ten skrót jest już przypisany do „${clashLabel}". Wybierz inny.`, 'error');
      const clashEl = document.querySelector(`.hotkey-value[data-action="${editingHotkey}"]`);
      if (clashEl) {
        clashEl.classList.remove('editing');
        clashEl.classList.add('clash');
        clashEl.textContent = hotkeyString;
        setTimeout(() => {
          clashEl.classList.remove('clash');
          clashEl.textContent = bindings[editingHotkey as string] || '(nie ustawiono)';
        }, 1200);
      }
      editingHotkey = null;
      return;
    }
  }
  
  const valueEl = document.querySelector(`.hotkey-value[data-action="${editingHotkey}"]`);
  if (valueEl) {
    valueEl.textContent = hotkeyString;
    valueEl.classList.remove('editing');
  }
  
  if (currentConfig) {
    currentConfig.key_bindings[editingHotkey!] = hotkeyString;
    updateConfig(currentConfig).catch(err => {
      console.error('Failed to update hotkey:', err);
      showToast('Błąd aktualizacji skrótu', 'error');
    });
  }
  
  editingHotkey = null;
}

// Update config on backend
async function updateConfig(config: Partial<AppConfig>) {
  try {
    // Tauri maps Rust param `new_config` to JS key `newConfig`
    await invoke('update_config', { newConfig: config });
  } catch (error) {
    console.error('Update config error:', error);
    throw error;
  }
}

// Gather config from UI
function gatherConfigFromUI(): Partial<AppConfig> {
  const config: Partial<AppConfig> = {};
  
  // Files
  config.audio_dir = (document.getElementById('audio-dir') as HTMLInputElement).value;
  config.text_file_path = (document.getElementById('text-file') as HTMLInputElement).value;
  config.names_file_path = (document.getElementById('names-file') as HTMLInputElement).value;
  config.screenshot_dir = (document.getElementById('screenshot-dir') as HTMLInputElement).value;
  
  // Audio (slider is 0-100%, backend expects 0.0-1.0)
  config.VOLUME_REDUCTION_LEVEL =
    parseFloat((document.getElementById('volume-reduction') as HTMLInputElement).value) / 100;
  config.READER_VOLUME =
    parseFloat((document.getElementById('reader-volume') as HTMLInputElement).value) / 100;
  // output2 + dynamic-speed toggles removed from UI. Dynamic speed is forced
  // on so the single "Prędkość odtwarzania" slider acts as the user's manual
  // audio speed. Overlap speed (when lines overlap) is always 5% faster.
  config.ENABLE_DYNAMIC_SPEED = true;
  const playbackSpeed = parseFloat((document.getElementById('playback-speed') as HTMLInputElement).value);
  config.BASE_PLAYBACK_SPEED = playbackSpeed;
  // Overlap speed is set independently, but must stay at least 5% above base.
  const minOverlap = Math.round(playbackSpeed * 1.05 * 100) / 100;
  let overlapSpeed = parseFloat((document.getElementById('overlap-speed') as HTMLInputElement).value);
  if (!Number.isFinite(overlapSpeed) || overlapSpeed < minOverlap) overlapSpeed = minOverlap;
  config.OVERLAP_PLAYBACK_SPEED = overlapSpeed;
  const queueRadio = document.querySelector<HTMLInputElement>('input[name="queue-size"]:checked');
  if (queueRadio) {
    config.AUDIO_QUEUE_SIZE = parseInt(queueRadio.value);
  }
  config.MINIMIZE_TO_TRAY_ON_READER_START = (document.getElementById('minimize-to-tray') as HTMLInputElement).checked;
  
  // OCR
  config.RESOLUTION_DOWNSCALE = 1.0; // Fixed at full resolution for best OCR accuracy
  config.CAPTURE_INTERVAL = parseFloat((document.getElementById('capture-interval') as HTMLInputElement).value);
  config.CAPTURE_MODE = 'window'; // mode selector removed; always window capture
  config.CAPTURE_WINDOW_QUERY = (document.getElementById('capture-window-query') as HTMLInputElement).value;
  const monitorSelEl = document.getElementById('capture-monitor') as HTMLSelectElement | null;
  if (monitorSelEl) config.CAPTURE_MONITOR = monitorSelEl.value;
  config.MIN_HEIGHT = parseInt((document.getElementById('min-height') as HTMLInputElement).value);
  config.MAX_HEIGHT = parseInt((document.getElementById('max-height') as HTMLInputElement).value);
  config.ENABLE_REMOVE_CHARACTER_NAME = (document.getElementById('enable-remove-character-name') as HTMLInputElement).checked;
  config.ENABLE_SCREENSHOTS = (document.getElementById('enable-screenshots') as HTMLInputElement).checked;
  config.ENABLE_PARAGRAPH_OCR = (document.getElementById('enable-paragraph-ocr') as HTMLInputElement).checked;
  config.ENABLE_TYPEWRITER_WAIT = (document.getElementById('enable-typewriter-wait') as HTMLInputElement).checked;
  config.ENABLE_REGION_OVERLAY = (document.getElementById('enable-region-overlay') as HTMLInputElement).checked;
  const outlineModeGet = document.getElementById('enable-outline-text-mode') as HTMLInputElement | null;
  if (outlineModeGet) config.ENABLE_OUTLINE_TEXT_MODE = outlineModeGet.checked;
  config.USE_CENTER_LINE_1 = (document.getElementById('use-center-line-1') as HTMLInputElement).checked;
  config.USE_CENTER_LINE_2 = (document.getElementById('use-center-line-2') as HTMLInputElement).checked;
  config.USE_CENTER_LINE_3 = (document.getElementById('use-center-line-3') as HTMLInputElement).checked;
  config.CENTER_LINE_2_START = parseInt((document.getElementById('center-line-2-start') as HTMLInputElement).value);
  config.CENTER_LINE_3_START_RATIO = parseFloat((document.getElementById('center-line-3-ratio') as HTMLInputElement).value);
  config.CENTER_LINE_MARGIN = parseInt((document.getElementById('center-line-margin') as HTMLInputElement).value);
  
  // Advanced
  // (config.resolution is intentionally NOT set here: the region base
  // resolution is preserved from the loaded preset/JSON and never overwritten
  // from the UI, so shared 4K / 1080p / ultrawide presets keep their base.)

  config.monitor = {
    top: parseInt((document.getElementById('monitor-top') as HTMLInputElement).value),
    left: parseInt((document.getElementById('monitor-left') as HTMLInputElement).value),
    width: parseInt((document.getElementById('monitor-width') as HTMLInputElement).value),
    height: parseInt((document.getElementById('monitor-height') as HTMLInputElement).value)
  };
  
  config.monitor2_enabled = (document.getElementById('monitor2-enabled') as HTMLInputElement).checked;
  config.monitor2_top = parseInt((document.getElementById('monitor2-top') as HTMLInputElement).value);
  config.monitor2_left = parseInt((document.getElementById('monitor2-left') as HTMLInputElement).value);
  config.monitor2_width = parseInt((document.getElementById('monitor2-width') as HTMLInputElement).value);
  config.monitor2_height = parseInt((document.getElementById('monitor2-height') as HTMLInputElement).value);
  
  return config;
}

// Setup config change listeners
function setupConfigListeners() {
  const inputs = document.querySelectorAll('input, select');
  
  let sharedTimeout: number;
  const debouncedSave = () => {
    if (suppressConfigSave || !configLoaded) return;
    clearTimeout(sharedTimeout);
    sharedTimeout = setTimeout(async () => {
      const config = gatherConfigFromUI();
      try {
        await updateConfig(config);
      } catch (error) {
        console.error('Failed to update config:', error);
      }
    }, 500) as unknown as number;
  };

  const immediateSave = async () => {
    if (suppressConfigSave || !configLoaded) return;
    const config = gatherConfigFromUI();
    try {
      await updateConfig(config);
    } catch (error) {
      console.error('Failed to update config:', error);
    }
  };

  inputs.forEach(input => {
    const id = input.id;
    if (id === 'capture-window-select' || id === 'capture-window-query') return;
    if (id === 'capture-monitor') return;
    
    if (input.tagName === 'INPUT' && 
        ((input as HTMLInputElement).type === 'text' || 
         (input as HTMLInputElement).type === 'range' ||
         (input as HTMLInputElement).type === 'number')) {
      input.addEventListener('input', debouncedSave);
    } else {
      input.addEventListener('change', immediateSave);
    }
  });
}

// Show/hide the on-screen OCR region frame
function setupRegionOverlayToggle() {
  const checkbox = document.getElementById('enable-region-overlay') as HTMLInputElement | null;
  if (!checkbox) return;

  checkbox.addEventListener('change', async () => {
    try {
      await invoke('set_region_overlay', { visible: checkbox.checked });
    } catch (error) {
      console.error('Failed to toggle OCR region overlay:', error);
      showToast('Błąd ramki obszaru OCR', 'error');
    }
  });
}

// Enable/disable reader
async function toggleReader() {
  const btn = document.getElementById('enable-reader-btn');
  if (!btn) return;
  
  try {
    if (readerEnabled) {
      await invoke('disable_reader');
    } else {
      // Sync full config from UI before starting so nothing is stale
      const cfg = gatherConfigFromUI();

      // No game window specified -> let the user pick one (alt+tab style with
      // live thumbnails), but only at the moment they actually start.
      if (!cfg.CAPTURE_WINDOW_QUERY || !cfg.CAPTURE_WINDOW_QUERY.trim()) {
        const picked = await invoke<{ query: string; title: string } | null>('pick_window');
        if (!picked) return; // cancelled -> don't start
        cfg.CAPTURE_WINDOW_QUERY = picked.query;
        (document.getElementById('capture-window-query') as HTMLInputElement).value = picked.query;
        setActiveWindowInfo(picked.title);
        updateWindowResolutionInfo();
      }

      try {
        await updateConfig(cfg);
      } catch (e) {
        console.error('Failed to sync config before start:', e);
      }
      await invoke('enable_reader');
    }
  } catch (error) {
    console.error('Toggle reader error:', error);
    showToast(String(error), 'error');
  }
}

// Load recent presets
async function loadRecentPresets() {
  try {
    const presets = await invoke<RecentPreset[]>('get_recent_presets');
    renderRecentPresets(presets);
  } catch (error) {
    console.error('Failed to load recent presets:', error);
  }
}

// Render recent presets
function renderRecentPresets(presets: RecentPreset[]) {
  const container = document.getElementById('recent-presets');
  if (!container) return;
  
  if (presets.length === 0) {
    container.innerHTML = '<p class="empty-state">Brak ostatnich presetów</p>';
    return;
  }
  
  container.innerHTML = '';
  
  presets.forEach(preset => {
    const item = document.createElement('div');
    item.className = 'preset-item';
    
    const info = document.createElement('div');
    const name = document.createElement('div');
    name.className = 'preset-name';
    name.textContent = preset.name;
    
    const path = document.createElement('div');
    path.className = 'preset-path';
    path.textContent = preset.path;
    
    info.appendChild(name);
    info.appendChild(path);
    
    const loadBtn = document.createElement('button');
    loadBtn.className = 'btn btn-secondary preset-load-btn';
    loadBtn.textContent = 'Wczytaj';
    loadBtn.addEventListener('click', async () => {
      try {
        await invoke('load_preset', { path: preset.path });
        await loadInitialConfig();
        setActivePresetInfo(preset.name);
        showToast(`Wczytano preset: ${preset.name}`, 'success');
      } catch (error) {
        console.error('Failed to load preset:', error);
        showToast('Błąd wczytywania presetu', 'error');
      }
    });

    const deleteBtn = document.createElement('button');
    deleteBtn.className = 'btn btn-secondary preset-delete-btn';
    deleteBtn.textContent = 'Usuń';
    deleteBtn.addEventListener('click', async () => {
      try {
        await invoke('remove_recent_preset', { path: preset.path });
        showToast(`Usunięto z listy: ${preset.name}`, 'success');
        await loadRecentPresets();
      } catch (error) {
        console.error('Failed to remove preset:', error);
        showToast('Błąd usuwania presetu z listy', 'error');
      }
    });

    item.appendChild(info);
    item.appendChild(loadBtn);
    item.appendChild(deleteBtn);
    container.appendChild(item);
  });
}

// Show which preset is currently active.
function setActivePresetInfo(name: string | null) {
  const presetCard = document.getElementById('preset-card-value');
  if (presetCard) presetCard.textContent = name || '--';
}

// Preset save/load buttons
function setupPresetButtons() {
  document.getElementById('load-preset-btn')?.addEventListener('click', async () => {
    try {
      const selected = await open({
        filters: [{
          name: 'JSON',
          extensions: ['json']
        }],
        title: 'Wczytaj preset'
      });
      
      if (selected && typeof selected === 'string') {
        await invoke('load_preset', { path: selected });
        await loadInitialConfig();
        const name = selected.replace(/^.*[\\/]/, '').replace(/\.json$/i, '');
        setActivePresetInfo(name);
        await loadRecentPresets();
        showToast(`Preset wczytany: ${name}`, 'success');
      }
    } catch (error) {
      console.error('Load preset error:', error);
      showToast('Błąd wczytywania presetu', 'error');
    }
  });
  
  document.getElementById('save-preset-btn')?.addEventListener('click', async () => {
    try {
      const selected = await save({
        filters: [{
          name: 'JSON',
          extensions: ['json']
        }],
        defaultPath: 'preset.json',
        title: 'Zapisz preset'
      });

      if (selected && typeof selected === 'string') {
        await invoke('save_preset', { path: selected });
        const name = selected.replace(/^.*[\\/]/, '').replace(/\.json$/i, '');
        setActivePresetInfo(name);
        await loadRecentPresets();
        showToast(`Preset zapisany: ${name}`, 'success');
      }
    } catch (error) {
      console.error('Save preset error:', error);
      showToast('Błąd zapisywania presetu', 'error');
    }
  });
}

// Listen to backend events
function setupEventListeners() {
  // Reader state changes
  listen<{ enabled: boolean }>('reader_state', (event) => {
    readerEnabled = event.payload.enabled;
    const btn = document.getElementById('enable-reader-btn');
    if (btn) {
      btn.textContent = readerEnabled ? 'ZATRZYMAJ' : 'URUCHOM';
      btn.className = readerEnabled ? 'run-btn-sidebar is-active' : 'run-btn-sidebar';
    }
    const statusCard = document.getElementById('status-card-value');
    if (statusCard) statusCard.textContent = readerEnabled ? 'Aktywny' : 'Gotowy';
    document.body.classList.toggle('reader-active', readerEnabled);
  });
  
  // Validation errors
  listen<{ error: string }>('validation_error', (event) => {
    showToast(event.payload.error, 'error');
  });
  
  // Preset loaded
  listen<{ name: string }>('preset_loaded', async (event) => {
    showToast(`Wczytano preset: ${event.payload.name}`, 'success');
    setActivePresetInfo(event.payload.name);
    // Reload config into UI
    await loadInitialConfig();
    await loadRecentPresets();
  });
  
  // Debug logs
  listen<{ level: string; message: string }>('debug_log', (event) => {    console.log(`[${event.payload.level}] ${event.payload.message}`);
  });

  // User-facing notices (e.g. preset shortcuts auto-fixed)
  listen<{ message: string }>('notice', (event) => {
    showToast(event.payload.message, 'warning');
  });

  // Both PL and EN subtitle files were found in the preset folder — ask which.
  listen<{ pl: string; en: string }>('subtitle_choice', async (event) => {
    const { pl, en } = event.payload;
    const usePl = await chooseSubtitleLanguage();
    const chosen = usePl ? pl : en;
    const input = document.getElementById('text-file') as HTMLInputElement | null;
    if (input) input.value = chosen;
    if (currentConfig) currentConfig.text_file_path = chosen;
    try {
      await updateConfig(gatherConfigFromUI());
    } catch (e) {
      console.error('Failed to persist subtitle choice:', e);
    }
    showToast(`Wybrano napisy: ${usePl ? 'polskie' : 'angielskie'}`, 'success');
  });

  // Region overlay toggled via hotkey (alt+2) -> sync the checkbox.
  listen<boolean>('region_overlay_changed', (event) => {
    const cb = document.getElementById('enable-region-overlay') as HTMLInputElement | null;
    if (cb) cb.checked = event.payload;
    if (currentConfig) currentConfig.ENABLE_REGION_OVERLAY = event.payload;
  });

  // Config changed on the backend (e.g. a hotkey adjusted volume/speed) ->
  // update just those controls (no full UI reload, so nothing else flickers
  // or gets reset).
  listen<{ volume_reduction_level: number; reader_volume: number; base_playback_speed: number; overlap_playback_speed: number }>(
    'config_changed',
    (event) => {
      const p = event.payload;
      suppressConfigSave = true;
      try {
        const vol = document.getElementById('volume-reduction') as HTMLInputElement | null;
        if (vol) {
          vol.value = String(Math.round(p.volume_reduction_level * 100));
          vol.dispatchEvent(new Event('input', { bubbles: true }));
        }
        const rv = document.getElementById('reader-volume') as HTMLInputElement | null;
        if (rv) {
          rv.value = String(Math.round(p.reader_volume * 100));
          rv.dispatchEvent(new Event('input', { bubbles: true }));
        }
        const base = document.getElementById('playback-speed') as HTMLInputElement | null;
        if (base) {
          base.value = p.base_playback_speed.toFixed(2);
          base.dispatchEvent(new Event('input', { bubbles: true }));
        }
        const overlap = document.getElementById('overlap-speed') as HTMLInputElement | null;
        if (overlap) {
          overlap.value = p.overlap_playback_speed.toFixed(2);
          overlap.dispatchEvent(new Event('input', { bubbles: true }));
        }
        if (currentConfig) {
          currentConfig.VOLUME_REDUCTION_LEVEL = p.volume_reduction_level;
          currentConfig.READER_VOLUME = p.reader_volume;
          currentConfig.BASE_PLAYBACK_SPEED = p.base_playback_speed;
          currentConfig.OVERLAP_PLAYBACK_SPEED = p.overlap_playback_speed;
        }
      } finally {
        suppressConfigSave = false;
      }
    }
  );
}

// Load initial config from backend
async function loadInitialConfig() {
  // The backend registers its state near the end of setup(); the webview may
  // call get_config before that finishes, which fails intermittently ("brak
  // konfiguracji" + empty hotkeys). Retry a few times until the state is ready.
  let config: AppConfig | null = null;
  for (let attempt = 1; attempt <= 15; attempt++) {
    try {
      config = await invoke<AppConfig>('get_config');
      break;
    } catch (error) {
      if (attempt === 15) {
        console.error('Failed to load initial config after retries:', error);
      }
      await new Promise((r) => setTimeout(r, 200));
    }
  }
  if (!config) {
    showToast('Błąd wczytywania konfiguracji', 'error');
    return;
  }
  try {
    loadConfigIntoUI(config);
  } catch (error) {
    console.error('Failed to apply config to UI:', error);
    showToast('Błąd wczytywania konfiguracji', 'error');
  }
}

// Initialize app
// Window info from backend list_windows command
interface WindowInfo {
  title: string;
  process_name: string;
  hwnd: string;
}

// Populate the window dropdown from the backend.
async function populateWindowList() {
  const select = document.getElementById('capture-window-select') as HTMLSelectElement;
  if (!select) return;
  try {
    const windows = await invoke<WindowInfo[]>('list_windows');
    const current = select.value;
    select.innerHTML = '<option value="">— wybierz okno —</option>';
    for (const w of windows) {
      const opt = document.createElement('option');
      // Prefer matching by process exe name (stable across sessions)
      opt.value = w.process_name || w.title;
      const procLabel = w.process_name ? ` [${w.process_name}]` : '';
      opt.textContent = `${w.title}${procLabel}`;
      select.appendChild(opt);
    }
    select.value = current;
  } catch (err) {
    console.error('Failed to list windows:', err);
  }
}

// Wire up capture mode controls.
function initializeCaptureMode() {
  const windowSelect = document.getElementById('capture-window-select') as HTMLSelectElement;
  const queryInput = document.getElementById('capture-window-query') as HTMLInputElement;
  const refreshBtn = document.getElementById('refresh-windows-btn');

  // Always populate window list on init (mode is always "window")
  populateWindowList();

  // Picking a window from the dropdown fills the query field (process name)
  // and shows the window's resolution (display only).
  windowSelect?.addEventListener('change', () => {
    if (windowSelect.value) {
      queryInput.value = windowSelect.value;
      queryInput.dispatchEvent(new Event('input'));
      updateWindowResolutionInfo();
      // Update the "Okno gry" card with the selected window title
      const selectedOption = windowSelect.options[windowSelect.selectedIndex];
      const windowTitle = selectedOption ? selectedOption.text.split(' [')[0] : windowSelect.value;
      setActiveWindowInfo(windowTitle);
    }
  });

  // Manually typing/editing the query also refreshes the resolution hint.
  queryInput?.addEventListener('change', () => {
    updateWindowResolutionInfo();
  });

  // Any manual edit (typing or dropdown) drops the exact pinned window from the
  // thumbnail picker, so name-based matching takes over again. The thumbnail
  // picker sets the value without dispatching 'input', so its pin survives.
  queryInput?.addEventListener('input', () => {
    invoke('clear_pinned_window').catch((e) => console.error('clear pin failed:', e));
    const cardEl = document.getElementById('window-card-value');
    if (cardEl) cardEl.textContent = queryInput.value.trim() || '--';
  });

  refreshBtn?.addEventListener('click', () => {
    populateWindowList();
    updateWindowResolutionInfo();
  });

  // Initial hint if a window is already configured.
  updateWindowResolutionInfo();
}

// Setup audio control buttons
function setupAudioControls() {
  const skipBtn = document.getElementById('skip-next-line-btn');
  
  skipBtn?.addEventListener('click', async () => {
    try {
      await invoke('skip_next_line');
      showToast('Pomijam do następnej linii (+10% prędkość)', 'success');
    } catch (error) {
      console.error('Failed to skip to next line:', error);
      showToast('Błąd podczas pomijania: ' + String(error), 'error');
    }
  });
}

// Casual hover tooltips for every option. Native WebView2 title tooltips: we set
// the title on the control and on its surrounding row/label so hovering anywhere
// on the option shows the hint.
function setupTooltips() {
  const tips: Record<string, string> = {
    // Sidebar / sterowanie
    'enable-reader-btn': 'Odpala albo zatrzymuje lektora. Jak nie ma podanego okna gry, najpierw poprosi o wybór.',
    'theme-toggle': 'Przełącza wygląd na jasny albo ciemny. Czysto kosmetyczne.',
    'capture-window-select': 'Lista otwartych okien. Wybierz tu grę z której ma czytać, jak nie chce ci się wpisywać nazwy.',
    'refresh-windows-btn': 'Odświeża listę okien. Klik jak odpaliłeś grę po otwarciu programu.',
    'capture-window-query': 'Nazwa procesu (np. GTA-SA.exe) albo kawałek tytułu okna. Po tym program wie skąd czytać i którą grę przyciszać.',
    'capture-monitor': 'Na którym ekranie program ma działać. Zostaw automat, to sam podąży za oknem gry.',
    // Presety
    'load-preset-btn': 'Wczytuje zapisane ustawienia z pliku. Lektor się przy tym zatrzyma, żeby nie czytał starego.',
    'save-preset-btn': 'Zapisuje aktualne ustawienia do pliku, żebyś nie ustawiał wszystkiego od nowa.',
    // Pliki
    'audio-dir': 'Folder z plikami audio (np. output1 (123).ogg). Stąd program bierze nagrania do odtworzenia.',
    'text-file': 'Plik .txt z dialogami. Do niego program dopasowuje tekst złapany z ekranu.',
    'names-file': 'Opcjonalny plik z imionami postaci. Pomaga obcinać imiona z początku linii.',
    'screenshot-dir': 'Gdzie lecą zrzuty ekranu z OCR (jak masz włączone). Przydaje się do podglądu co program widzi.',
    // OCR / przechwytywanie
    'capture-interval': 'Co ile sekund program robi zdjęcie ekranu. Mniej = szybciej reaguje ale więcej CPU.',
    'min-height': 'Najmniejsza wysokość tekstu (px) jaką bierze pod uwagę. Odsiewa drobny śmieciowy tekst.',
    'max-height': 'Największa wysokość tekstu (px). Odsiewa wielkie napisy typu logo czy tytuł.',
    'enable-remove-character-name': 'Wycina imię postaci z początku linii (np. "Tommy: cześć" → "cześć"). Działa automatycznie przez „:"/„;" (bez pliku) i przez listę imion z pliku.',
    'enable-screenshots': 'Zapisuje zrzuty tego co OCR widzi. Fajne do debugowania, ale obciąża dysk.',
    'enable-region-overlay': 'Pokazuje na ekranie ramkę dokładnie tam gdzie program czyta. Wygodne przy ustawianiu obszaru.',
    'enable-paragraph-ocr': 'Rozdziela osobne bloki tekstu na osobne dialogi. Włącz jak masz dwa obszary albo dwie wypowiedzi naraz.',
    'enable-typewriter-wait': 'Czeka aż tekst „domaszynuje" się do końca zanim go dopasuje. Dla gier gdzie litery pojawiają się po kolei.',
    'minimize-to-tray': 'Gdy włączysz lektora, okno chowa się do zasobnika (tray). Kliknij ikonę w zasobniku, żeby je przywrócić. Po zatrzymaniu lektora wraca samo.',
    // Region 1
    'select-region-btn': 'Zaznacz obszar myszką jak w Win+Shift+S. Program się schowa na czas zaznaczania.',
    'monitor-top': 'Górna krawędź obszaru czytania (px). Możesz wpisać ręcznie zamiast zaznaczać.',
    'monitor-left': 'Lewa krawędź obszaru czytania (px).',
    'monitor-width': 'Szerokość obszaru czytania (px).',
    'monitor-height': 'Wysokość obszaru czytania (px).',
    // Region 2
    'monitor2-enabled': 'Włącza drugi obszar czytania. Program czyta oba naraz jednym przebiegiem OCR.',
    'select-region2-btn': 'Zaznacz drugi obszar myszką, tak samo jak pierwszy.',
    'monitor2-top': 'Górna krawędź drugiego obszaru (px).',
    'monitor2-left': 'Lewa krawędź drugiego obszaru (px).',
    'monitor2-width': 'Szerokość drugiego obszaru (px).',
    'monitor2-height': 'Wysokość drugiego obszaru (px).',
    // Ręczne ustawianie obszaru
    'toggle-manual-region1': 'Rozwiń ręczne ustawianie współrzędnych pierwszego obszaru.',
    'toggle-manual-region2': 'Rozwiń ręczne ustawianie współrzędnych drugiego obszaru.',
    'win-min': 'Zwiń do zasobnika.',
    'win-max': 'Maksymalizuj okno.',
    'win-close': 'Zamknij program.',
    // Linie środkowe
    'use-center-line-1': 'Czyta tylko tekst w poziomej linii na środku ekranu. Odsiewa napisy z góry/dołu.',
    'use-center-line-2': 'Druga linia filtrująca, na pozycji którą ustawisz niżej.',
    'center-line-2-start': 'Na jakiej wysokości (px) leży druga linia środkowa.',
    'use-center-line-3': 'Trzecia linia filtrująca, ustawiana proporcją wysokości ekranu.',
    'center-line-3-ratio': 'Gdzie leży trzecia linia: 0 to góra, 1 to dół ekranu.',
    'center-line-margin': 'Jak gruby jest pas wokół linii (px). Większy = łapie tekst z większego zakresu.',
    // Audio
    'reader-volume': 'Głośność samego lektora (czytanych nagrań). Sterujesz też skrótami PageUp/PageDown.',
    'volume-reduction': 'O ile przyciszyć grę gdy lektor mówi. 100% = gra prawie niema, 0% = gra na full.',
    'playback-speed': 'Bazowe tempo czytania nagrań. 1.0 = normalnie.',
    'overlap-speed': 'Tempo gdy lektor „dogania" zaległe linie. Musi być min. 5% wyżej od bazowego.',
  };

  let tipEl = document.getElementById('app-tooltip') as HTMLDivElement | null;
  if (!tipEl) {
    tipEl = document.createElement('div');
    tipEl.id = 'app-tooltip';
    tipEl.className = 'app-tooltip';
    document.body.appendChild(tipEl);
  }
  const tip = tipEl;
  let hideTimer: number | undefined;
  let showTimer: number | undefined;
  let rafId: number | undefined;
  let cachedRect: DOMRect | null = null;
  const SHOW_DELAY = 750;

  function positionTip(x: number, y: number) {
    const pad = 14;
    if (!cachedRect) cachedRect = tip.getBoundingClientRect();
    const w = cachedRect.width;
    const h = cachedRect.height;
    let left = x + 16;
    let top = y + 18;
    if (left + w + pad > window.innerWidth) left = x - w - 16;
    if (left < pad) left = pad;
    if (top + h + pad > window.innerHeight) top = y - h - 16;
    if (top < pad) top = pad;
    tip.style.left = `${left}px`;
    tip.style.top = `${top}px`;
  }

  function showTip(text: string, x: number, y: number) {
    if (hideTimer) { clearTimeout(hideTimer); hideTimer = undefined; }
    tip.textContent = text;
    cachedRect = null;
    tip.style.left = `${x + 16}px`;
    tip.style.top = `${y + 18}px`;
    tip.classList.add('visible');
    positionTip(x, y);
  }

  function hideTip() {
    if (showTimer) { clearTimeout(showTimer); showTimer = undefined; }
    if (rafId) { cancelAnimationFrame(rafId); rafId = undefined; }
    cachedRect = null;
    tip.classList.remove('visible');
  }

  for (const [id, text] of Object.entries(tips)) {
    const el = document.getElementById(id);
    if (!el) continue;
    const row = (el.closest('.field-row') || el.closest('.sw-row') || el.closest('.field-group') || el.closest('.form-group') || el.closest('.button-group')) as HTMLElement | null;
    const target = row ?? el;
    target.dataset.tip = text;
    target.removeAttribute('title');
    let lastX = 0, lastY = 0;
    target.addEventListener('mouseenter', (e) => {
      lastX = (e as MouseEvent).clientX;
      lastY = (e as MouseEvent).clientY;
      if (showTimer) clearTimeout(showTimer);
      showTimer = window.setTimeout(() => showTip(text, lastX, lastY), SHOW_DELAY);
    });
    target.addEventListener('mousemove', (e) => {
      lastX = (e as MouseEvent).clientX;
      lastY = (e as MouseEvent).clientY;
      if (tip.classList.contains('visible') && !rafId) {
        rafId = requestAnimationFrame(() => {
          positionTip(lastX, lastY);
          rafId = undefined;
        });
      }
    });
    target.addEventListener('mouseleave', hideTip);
  }

  // Queue-size radios share a name, not an id.
  const queueText = 'Ile dialogów (z pliku txt) może czekać w kolejce. Więcej = lektor nie gubi linii, ale bardziej się opóźnia.';
  const queueRow = document.querySelector<HTMLInputElement>('input[name="queue-size"]')
    ?.closest('.form-group') as HTMLElement | null;
  if (queueRow) {
    queueRow.dataset.tip = queueText;
    queueRow.removeAttribute('title');
    let lastX = 0, lastY = 0;
    queueRow.addEventListener('mouseenter', (e) => {
      lastX = (e as MouseEvent).clientX;
      lastY = (e as MouseEvent).clientY;
      if (showTimer) clearTimeout(showTimer);
      showTimer = window.setTimeout(() => showTip(queueText, lastX, lastY), SHOW_DELAY);
    });
    queueRow.addEventListener('mousemove', (e) => {
      lastX = (e as MouseEvent).clientX;
      lastY = (e as MouseEvent).clientY;
      if (tip.classList.contains('visible') && !rafId) {
        rafId = requestAnimationFrame(() => {
          positionTip(lastX, lastY);
          rafId = undefined;
        });
      }
    });
    queueRow.addEventListener('mouseleave', hideTip);
  }
}

// In-app modal asking PL vs EN subtitles. Resolves true=PL, false=EN.
function chooseSubtitleLanguage(): Promise<boolean> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.innerHTML = `
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal-title">Wybór języka napisów</div>
        <div class="modal-body">W folderze presetu są napisy polskie i angielskie. Których użyć?</div>
        <div class="modal-actions">
          <button type="button" class="btn primary" data-lang="pl">Polskie (PL)</button>
          <button type="button" class="btn" data-lang="en">Angielskie (EN)</button>
        </div>
      </div>`;
    const finish = (usePl: boolean) => {
      overlay.classList.remove('visible');
      setTimeout(() => overlay.remove(), 180);
      resolve(usePl);
    };
    overlay.querySelector('[data-lang="pl"]')?.addEventListener('click', () => finish(true));
    overlay.querySelector('[data-lang="en"]')?.addEventListener('click', () => finish(false));
    document.body.appendChild(overlay);
    requestAnimationFrame(() => overlay.classList.add('visible'));
  });
}

// Wire the custom title bar window controls (frameless window).
function setupTitlebar() {
  const appWin = getCurrentWindow();
  // Minimize to the taskbar (reliable restore by clicking the taskbar icon).
  document.getElementById('win-min')?.addEventListener('click', () => { appWin.minimize(); });
  document.getElementById('win-max')?.addEventListener('click', () => { appWin.toggleMaximize(); });
  // Close fully quits the app (stops reader, exits process).
  document.getElementById('win-close')?.addEventListener('click', async () => {
    try { await invoke('exit_app'); } catch (e) { console.error(e); }
  });
}

async function initializeApp() {
  console.log('Initializing GameReader UI...');
  
  // Setup UI
  hardenWebview();
  initializeTheme();
  setupTitlebar();
  initializeTabs();
  initializeSliders();
  setupSpeedSlider();
  initializePickers();
  setupKeyboardHandler();
  setupConfigListeners();
  setupPresetButtons();
  setupEventListeners();
  setupRegionOverlayToggle();
  initializeCaptureMode();
  setupRegionSelector();
  setupAudioControls();
  setupTooltips();
  
  // Populate monitor picker before loading config so the saved value restores.
  await setupMonitorPicker();

  // Load initial data
  await loadInitialConfig();
  await loadRecentPresets();

  // Show the currently active preset (from backend runtime state).
  try {
    const rt = await invoke<{ preset_filename?: string }>('get_runtime_state');
    if (rt?.preset_filename) setActivePresetInfo(rt.preset_filename);
  } catch (e) {
    console.error('get_runtime_state failed:', e);
  }
  
  // Restore OCR region overlay if it was enabled in the loaded config
  if (currentConfig?.ENABLE_REGION_OVERLAY) {
    try {
      await invoke('set_region_overlay', { visible: true });
    } catch (error) {
      console.error('Failed to restore OCR region overlay:', error);
    }
  }
  
  // Setup enable/disable button
  document.getElementById('enable-reader-btn')?.addEventListener('click', toggleReader);
  
  console.log('GameReader UI initialized');
}

// Start when DOM is ready
window.addEventListener('DOMContentLoaded', () => {
  initializeApp().catch(err => {
    console.error('Failed to initialize app:', err);
    showToast('Błąd inicjalizacji aplikacji', 'error');
  });
});
