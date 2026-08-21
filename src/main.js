const { invoke } = window.__TAURI__.core;
const { open, ask, save } = window.__TAURI__.dialog;
const { getCurrentWebview } = window.__TAURI__.webview;
const { getCurrentWindow } = window.__TAURI__.window;

let currentFilePath = null;
// Whether the form has edits that have not been written back to the archive.
let isDirty = false;
// Set while populateForm() writes values programmatically, so restoring a file
// into the form doesn't count as the user editing it.
let isPopulating = false;

// Fields backed by a floating-point type in Rust. Every other number input maps
// to an i32 and must never be sent a fractional value.
const DECIMAL_FIELDS = new Set(['community_rating']);

// DOM elements
const btnOpen = document.getElementById('btn-open');
const btnSave = document.getElementById('btn-save');
const fileName = document.getElementById('file-name');
const statusMessage = document.getElementById('status-message');
const coverPreview = document.getElementById('cover-preview');
const pageCount = document.getElementById('page-count');
const form = document.getElementById('comic-form');
const tabs = document.querySelectorAll('.tab');
const loadingOverlay = document.getElementById('loading-overlay');
const loadingText = document.getElementById('loading-text');
const dropZone = document.getElementById('drop-zone');
const btnTheme = document.getElementById('btn-theme');

// Field mapping from form IDs to ComicInfo JSON keys (PascalCase from Rust serde)
const fieldMap = {
    title: 'Title',
    series: 'Series',
    number: 'Number',
    count: 'Count',
    volume: 'Volume',
    alternate_series: 'AlternateSeries',
    alternate_number: 'AlternateNumber',
    alternate_count: 'AlternateCount',
    summary: 'Summary',
    notes: 'Notes',
    review: 'Review',
    year: 'Year',
    month: 'Month',
    day: 'Day',
    writer: 'Writer',
    penciller: 'Penciller',
    inker: 'Inker',
    colorist: 'Colorist',
    letterer: 'Letterer',
    cover_artist: 'CoverArtist',
    editor: 'Editor',
    translator: 'Translator',
    publisher: 'Publisher',
    imprint: 'Imprint',
    genre: 'Genre',
    tags: 'Tags',
    characters: 'Characters',
    teams: 'Teams',
    locations: 'Locations',
    main_character_or_team: 'MainCharacterOrTeam',
    story_arc: 'StoryArc',
    story_arc_number: 'StoryArcNumber',
    series_group: 'SeriesGroup',
    format: 'Format',
    page_count: 'PageCount',
    language_iso: 'LanguageISO',
    web: 'Web',
    gtin: 'GTIN',
    scan_information: 'ScanInformation',
    black_and_white: 'BlackAndWhite',
    manga: 'Manga',
    age_rating: 'AgeRating',
    community_rating: 'CommunityRating'
};

// Initialize
window.addEventListener('DOMContentLoaded', async () => {
    setupTheme();
    setupTabs();
    setupButtons();
    setupDragDrop();
    setupDirtyTracking();
    const version = await window.__TAURI__.app.getVersion();
    document.getElementById('app-version').textContent = `v${version}`;

    // Register the listener and wait for the Rust event system to confirm it,
    // then tell the backend we're ready. The backend will immediately emit any
    // file that arrived via "Open With" before the frontend was loaded.
    await window.__TAURI__.event.listen('open-file', (event) => {
        handleFilePath(event.payload);
    });
    await invoke('frontend_ready');
});

function setupTheme() {
    // Load saved theme preference, default to dark
    const savedTheme = localStorage.getItem('theme') || 'dark';
    applyTheme(savedTheme);

    // Theme toggle button
    btnTheme.addEventListener('click', () => {
        const currentTheme = document.documentElement.getAttribute('data-theme') || 'dark';
        const newTheme = currentTheme === 'dark' ? 'light' : 'dark';
        applyTheme(newTheme);
        localStorage.setItem('theme', newTheme);
    });
}

function applyTheme(theme) {
    if (theme === 'light') {
        document.documentElement.setAttribute('data-theme', 'light');
    } else {
        document.documentElement.removeAttribute('data-theme');
    }
}

function setupTabs() {
    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            const tabName = tab.dataset.tab;

            // Update active tab
            tabs.forEach(t => t.classList.remove('active'));
            tab.classList.add('active');

            // Update visible content
            document.querySelectorAll('.tab-content').forEach(content => {
                content.classList.remove('active');
            });
            document.getElementById(`tab-${tabName}`).classList.add('active');
        });
    });
}

function setupButtons() {
    btnOpen.addEventListener('click', openFile);
    btnSave.addEventListener('click', saveFile);
}

function setupDirtyTracking() {
    const onEdit = () => {
        if (isPopulating || isDirty) return;
        isDirty = true;
        updateFileNameDisplay();
    };
    form.addEventListener('input', onEdit);
    form.addEventListener('change', onEdit);

    // Closing the window would otherwise discard edits without a word.
    getCurrentWindow().onCloseRequested(async (event) => {
        if (!isDirty) return;
        event.preventDefault();
        if (await confirmDiscardChanges()) {
            isDirty = false;
            await getCurrentWindow().destroy();
        }
    });
}

/// Ask before throwing away edits. Returns true when it is safe to proceed.
async function confirmDiscardChanges() {
    if (!isDirty) return true;
    return await ask(
        'This file has unsaved changes. Discard them?',
        { title: 'Unsaved Changes', kind: 'warning' }
    );
}

function setDirty(dirty) {
    isDirty = dirty;
    updateFileNameDisplay();
}

function updateFileNameDisplay() {
    if (!currentFilePath) {
        fileName.textContent = 'No file loaded';
        fileName.title = 'No file loaded';
        return;
    }
    const displayName = currentFilePath.split('/').pop().split('\\').pop();
    fileName.textContent = isDirty ? `${displayName} •` : displayName;
    fileName.title = currentFilePath;
}

function setupDragDrop() {
    getCurrentWebview().onDragDropEvent(async (event) => {
        if (event.payload.type === 'over') {
            dropZone.classList.remove('hidden');
        } else if (event.payload.type === 'leave' || event.payload.type === 'cancel') {
            dropZone.classList.add('hidden');
        } else if (event.payload.type === 'drop') {
            dropZone.classList.add('hidden');
            const paths = event.payload.paths;
            if (paths.length === 0) return;
            if (paths.length > 1) {
                // The editor shows one file at a time; say so rather than
                // opening the first one as if that were the whole request.
                setStatus(`Dropped ${paths.length} files — opening only the first. Editing multiple files at once is not supported yet.`);
            }
            await handleFilePath(paths[0]);
        }
    });
}

async function handleFilePath(path, { discardConfirmed = false } = {}) {
    const lower = path.toLowerCase();
    const isZip = lower.endsWith('.cbz') || lower.endsWith('.zip');

    let format = null;
    if (lower.endsWith('.cbr') || lower.endsWith('.rar')) {
        format = 'RAR';
    } else if (lower.endsWith('.cb7') || lower.endsWith('.7z')) {
        format = '7-Zip';
    } else if (!isZip) {
        setStatus('Error: Unsupported file format. Open a CBZ, ZIP, CBR/RAR, or CB7/7z archive.');
        return;
    }

    // Check before doing any work, so the user isn't asked to convert a file
    // and only then told their edits are about to vanish.
    if (!discardConfirmed && !await confirmDiscardChanges()) {
        setStatus('Open cancelled — your unsaved changes were kept.');
        return;
    }

    if (isZip) {
        await openFileByPath(path);
        return;
    }

    const confirmed = await ask(
        `This file is in ${format} format. Convert it to CBZ?`,
        { title: 'Convert to CBZ', kind: 'info' }
    );

    if (!confirmed) {
        setStatus(`Conversion declined — ${format} archives must be converted to CBZ before editing.`);
        return;
    }

    // Resolve where the converted file will go, prompting before overwriting.
    let destPath;
    try {
        const target = await invoke('get_conversion_target', { sourcePath: path });
        destPath = target.path;
        if (target.exists) {
            const name = destPath.split('/').pop().split('\\').pop();
            const replace = await ask(
                `"${name}" already exists. Replace it?`,
                { title: 'File Exists', kind: 'warning' }
            );
            if (!replace) {
                const chosen = await save({
                    title: 'Save Converted CBZ As',
                    defaultPath: destPath,
                    filters: [{ name: 'Comic Archive', extensions: ['cbz'] }]
                });
                if (!chosen) {
                    setStatus('Conversion cancelled.');
                    return;
                }
                destPath = chosen;
            }
        }
    } catch (err) {
        setStatus(`Error: ${err}`);
        console.error(err);
        return;
    }

    try {
        showLoading('Converting to CBZ...');
        const newPath = await invoke('convert_to_cbz', { sourcePath: path, destPath });
        hideLoading();
        await openFileByPath(newPath);
    } catch (err) {
        hideLoading();
        const msg = String(err).includes('unar is not installed')
            ? "RAR conversion requires unar. Install it (e.g. 'brew install unar' on macOS)."
            : `Conversion failed: ${err}`;
        setStatus(msg);
        console.error(err);
    }
}

async function openFile() {
    try {
        if (!await confirmDiscardChanges()) {
            setStatus('Open cancelled — your unsaved changes were kept.');
            return;
        }

        const selected = await open({
            filters: [{
                name: 'Comic Archives',
                extensions: ['cbz', 'zip', 'cbr', 'rar', '7z', 'cb7']
            }]
        });

        if (!selected) return;
        await handleFilePath(selected, { discardConfirmed: true });
    } catch (err) {
        hideLoading();
        setStatus(`Error: ${err}`);
        console.error(err);
    }
}

async function openFileByPath(path) {
    try {
        showLoading('Opening file...');

        // Single backend call: comic info, page count, and cover from one
        // archive open (instead of three separate opens/scans).
        const { comicInfo, pageCount: pages, cover } = await invoke('open_cbz', { path });
        populateForm(comicInfo);

        // Only adopt the path once the archive actually opened — otherwise a
        // failed open leaves Save pointed at a file we never read.
        currentFilePath = path;
        setDirty(false);
        btnSave.disabled = false;

        // Cover and page count came back with the open call
        showCover(cover);
        pageCount.textContent = pages;

        hideLoading();
        setStatus('File loaded successfully');
    } catch (err) {
        hideLoading();
        setStatus(`Error: ${err}`);
        console.error(err);
    }
}

async function saveFile() {
    if (!currentFilePath) return;

    // The min/max/step attributes only bite on form submission, which never
    // happens here — so check them explicitly before writing anything.
    const invalid = findInvalidField();
    if (invalid) {
        revealField(invalid);
        setStatus(`Cannot save: ${invalid.labelText} — ${invalid.element.validationMessage}`);
        return;
    }

    btnSave.disabled = true;
    btnSave.textContent = 'Saving...';
    try {
        showLoading('Saving file...');
        const comicInfo = collectFormData();
        await invoke('save_cbz', { path: currentFilePath, comicInfo });
        hideLoading();
        setDirty(false);
        setStatus('File saved successfully');
    } catch (err) {
        hideLoading();
        setStatus(`Error: ${err}`);
        console.error(err);
    } finally {
        btnSave.disabled = false;
        btnSave.textContent = 'Save';
    }
}

/// First field whose value violates its own min/max/step constraints, or null.
function findInvalidField() {
    for (const formId of Object.keys(fieldMap)) {
        const element = document.getElementById(formId);
        if (!element || element.checkValidity()) continue;
        const label = document.querySelector(`label[for="${formId}"]`);
        return { element, labelText: label ? label.textContent : formId };
    }
    return null;
}

/// Bring a field into view: switch to its tab, focus it, and highlight it.
function revealField({ element }) {
    const pane = element.closest('.tab-content');
    if (pane) {
        const tabName = pane.id.replace(/^tab-/, '');
        const tab = document.querySelector(`.tab[data-tab="${tabName}"]`);
        if (tab) tab.click();
    }
    element.focus();
    element.reportValidity();
}

function showCover(coverData) {
    if (coverData) {
        coverPreview.innerHTML = `<img src="${coverData}" alt="Cover">`;
    } else {
        coverPreview.innerHTML = '<div class="cover-placeholder">No Cover</div>';
    }
}

function populateForm(data) {
    // Writing these values back is not a user edit.
    isPopulating = true;
    try {
        // Clear form first
        form.reset();

        for (const [formId, dataKey] of Object.entries(fieldMap)) {
            const element = document.getElementById(formId);
            if (!element) continue;

            const value = data[dataKey];

            if (value === null || value === undefined) {
                element.value = '';
                continue;
            }

            // Serde sends the YesNo/Manga/AgeRating enums as plain strings whose
            // text matches the corresponding <option value>. 'Unknown' has no
            // option of its own — it is the blank one at the top of each list.
            if (element.tagName === 'SELECT' && value === 'Unknown') {
                element.value = '';
            } else {
                element.value = value;
            }
        }
    } finally {
        isPopulating = false;
    }
}

function collectFormData() {
    const data = {};

    for (const [formId, dataKey] of Object.entries(fieldMap)) {
        const element = document.getElementById(formId);
        if (!element) continue;

        const value = element.value.trim();

        if (value === '') {
            data[dataKey] = null;
            continue;
        }

        // Handle different input types
        if (element.type === 'number') {
            // Everything except the DECIMAL_FIELDS maps to an i32 in the
            // backend, so a fractional value there would fail to save. The
            // form's step="1" constraint has already rejected those by the
            // time we get here, checked in findInvalidField().
            const num = Number(value);
            if (!Number.isFinite(num)) {
                data[dataKey] = null;
            } else {
                data[dataKey] = DECIMAL_FIELDS.has(formId) ? num : Math.trunc(num);
            }
        } else if (element.tagName === 'SELECT') {
            // Handle enum fields
            if (value === '') {
                data[dataKey] = null;
            } else {
                data[dataKey] = value;
            }
        } else {
            data[dataKey] = value;
        }
    }

    return data;
}

function setStatus(message) {
    statusMessage.textContent = message;
}

function showLoading(message = 'Loading...') {
    loadingText.textContent = message;
    loadingOverlay.classList.remove('hidden');
}

function hideLoading() {
    loadingOverlay.classList.add('hidden');
}
