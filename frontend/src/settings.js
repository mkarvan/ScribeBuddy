export class Settings {
    constructor() {
        this.appSelect = document.getElementById('app-select');
        this.modelSelect = document.getElementById('model-select');
        this.chunkSlider = document.getElementById('chunk-slider');
        this.chunkValueEl = document.getElementById('chunk-value');
        this.captureMode = document.getElementById('capture-mode');
        this.refreshBtn = document.getElementById('refresh-apps-btn');
        this.downloadBtn = document.getElementById('download-model-btn');
    }

    async loadApps() {
        try {
            const apps = await window.__TAURI__.core.invoke('list_running_apps');
            const selected = this.appSelect.value;
            this.appSelect.innerHTML = '<option value="">-- Select App --</option>';
            apps.forEach(app => {
                const option = document.createElement('option');
                option.value = app.bundle_id;
                option.textContent = app.name;
                this.appSelect.appendChild(option);
            });
            if (selected) this.appSelect.value = selected;
        } catch (err) {
            console.error('Failed to list apps:', err);
        }
    }

    async checkModel() {
        try {
            const available = await window.__TAURI__.core.invoke('check_model_available');
            this.downloadBtn.textContent = available ? '\u2713' : '\u21e9';
            this.downloadBtn.title = available ? 'Model ready' : 'Download model';
        } catch (err) {
            console.error('Failed to check model:', err);
        }
    }

    async downloadModel() {
        this.downloadBtn.disabled = true;
        this.downloadBtn.textContent = '...';
        try {
            await window.__TAURI__.core.invoke('download_model');
        } catch (err) {
            console.error('Download failed:', err);
        }
        this.downloadBtn.disabled = false;
        await this.checkModel();
    }

    async selectApp(bundleId) {
        await window.__TAURI__.core.invoke('set_target_app', { bundleId });
    }

    async setModel(size) {
        await window.__TAURI__.core.invoke('set_model_size', { size });
        await this.checkModel();
    }

    async setChunkDuration(secs) {
        await window.__TAURI__.core.invoke('set_chunk_duration', { seconds: parseFloat(secs) });
    }

    async setCaptureMode(useSCK) {
        await window.__TAURI__.core.invoke('set_capture_mode', { useScreencapturekit: useSCK });
    }

    bind() {
        this.appSelect.addEventListener('change', () => {
            this.selectApp(this.appSelect.value);
        });

        this.modelSelect.addEventListener('change', () => {
            this.setModel(this.modelSelect.value);
        });

        this.chunkSlider.addEventListener('input', () => {
            const val = this.chunkSlider.value;
            this.chunkValueEl.textContent = `${val}s`;
            this.setChunkDuration(val);
        });

        this.captureMode.addEventListener('change', () => {
            this.setCaptureMode(this.captureMode.value === 'sck');
        });

        this.refreshBtn.addEventListener('click', () => this.loadApps());

        this.downloadBtn.addEventListener('click', () => this.downloadModel());
    }
}
