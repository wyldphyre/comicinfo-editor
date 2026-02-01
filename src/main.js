const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { getCurrentWebview } = window.__TAURI__.webview;

let currentFilePath = null;

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
window.addEventListener('DOMContentLoaded', () => {
    setupTheme();
    setupTabs();
    setupButtons();
    setupDragDrop();
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

function setupDragDrop() {
    getCurrentWebview().onDragDropEvent(async (event) => {
        if (event.payload.type === 'over') {
            dropZone.classList.remove('hidden');
        } else if (event.payload.type === 'leave' || event.payload.type === 'cancel') {
            dropZone.classList.add('hidden');
        } else if (event.payload.type === 'drop') {
            dropZone.classList.add('hidden');
            const paths = event.payload.paths;
            if (paths.length > 0) {
                const path = paths[0];
                if (path.toLowerCase().endsWith('.cbz')) {
                    await openFileByPath(path);
                } else {
                    setStatus('Error: Please drop a CBZ file');
                }
            }
        }
    });
}

async function openFile() {
    try {
        const selected = await open({
            filters: [{
                name: 'Comic Archives',
                extensions: ['cbz']
            }]
        });

        if (!selected) return;
        await openFileByPath(selected);
    } catch (err) {
        hideLoading();
        setStatus(`Error: ${err}`);
        console.error(err);
    }
}

async function openFileByPath(path) {
    try {
        currentFilePath = path;
        showLoading('Opening file...');

        // Load comic info
        const comicInfo = await invoke('open_cbz', { path });
        populateForm(comicInfo);

        // Update UI
        const displayName = path.split('/').pop().split('\\').pop();
        fileName.textContent = displayName;
        fileName.title = path;  // Full path as tooltip
        btnSave.disabled = false;

        // Load cover and page count
        await Promise.all([
            loadCover(path),
            loadPageCount(path)
        ]);

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

    try {
        showLoading('Saving file...');
        const comicInfo = collectFormData();
        await invoke('save_cbz', { path: currentFilePath, comicInfo });
        hideLoading();
        setStatus('File saved successfully');
    } catch (err) {
        hideLoading();
        setStatus(`Error: ${err}`);
        console.error(err);
    }
}

async function loadCover(path) {
    try {
        const coverData = await invoke('extract_cover', { path });
        coverPreview.innerHTML = `<img src="${coverData}" alt="Cover">`;
    } catch (err) {
        coverPreview.innerHTML = '<div class="cover-placeholder">No Cover</div>';
    }
}

async function loadPageCount(path) {
    try {
        const count = await invoke('get_page_count', { path });
        pageCount.textContent = count;
    } catch (err) {
        pageCount.textContent = '-';
    }
}

function populateForm(data) {
    // Clear form first
    form.reset();

    for (const [formId, dataKey] of Object.entries(fieldMap)) {
        const element = document.getElementById(formId);
        if (!element) continue;

        let value = data[dataKey];

        if (value === null || value === undefined) {
            element.value = '';
            continue;
        }

        // Handle enum types (YesNo, Manga, AgeRating)
        if (typeof value === 'object') {
            // It's an enum - get the variant name
            if (value === 'Unknown') {
                element.value = '';
            } else {
                element.value = value;
            }
        } else {
            element.value = value;
        }
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
            const num = parseFloat(value);
            data[dataKey] = isNaN(num) ? null : num;
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
