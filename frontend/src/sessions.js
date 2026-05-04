export class SessionHistory {
    constructor(transcriptRenderer, errorUI) {
        this.transcriptRenderer = transcriptRenderer;
        this.errorUI = errorUI;
        this._viewingTimestamp = null;
        this._liveSegments = [];

        this.sidebar = document.getElementById('session-sidebar');
        this.sessionList = document.getElementById('session-list');
        this.banner = document.getElementById('session-banner');
        this.bannerLabel = document.getElementById('session-banner-label');
        this.backBtn = document.getElementById('session-back-btn');
        this.historyBtn = document.getElementById('history-toggle-btn');
        this.closeBtn = document.getElementById('sidebar-close-btn');

        this.historyBtn.addEventListener('click', () => this.toggleSidebar());
        this.closeBtn.addEventListener('click', () => this.closeSidebar());
        this.backBtn.addEventListener('click', () => this.returnToLive());
    }

    isViewing() { return this._viewingTimestamp !== null; }

    toggleSidebar() {
        const isOpen = !this.sidebar.classList.contains('closed');
        if (isOpen) {
            this.closeSidebar();
        } else {
            this.sidebar.classList.remove('closed');
            this.refresh();
        }
    }

    closeSidebar() {
        this.sidebar.classList.add('closed');
    }

    async refresh() {
        this.sessionList.innerHTML = '<div class="session-empty">Loading...</div>';
        try {
            const sessions = await window.__TAURI__.core.invoke('list_sessions');
            this.sessionList.innerHTML = '';

            if (sessions.length === 0) {
                this.sessionList.innerHTML = '<div class="session-empty">No past sessions yet</div>';
                return;
            }

            sessions.forEach(meta => {
                this.sessionList.appendChild(this._makeItem(meta));
            });
        } catch (err) {
            this.sessionList.innerHTML = '';
            this.errorUI.show(`Could not load sessions: ${err}`);
        }
    }

    _makeItem(meta) {
        const date = new Date(meta.timestamp * 1000);
        const dateStr = date.toLocaleDateString(undefined, {
            month: 'short', day: 'numeric',
            hour: '2-digit', minute: '2-digit',
        });
        const dur = this._fmtDuration(meta.duration_secs);

        const el = document.createElement('div');
        el.className = 'session-item';
        el.dataset.timestamp = meta.timestamp;
        el.innerHTML = `
            <div class="session-item-info">
                <div class="session-item-date">${dateStr}</div>
                <div class="session-item-meta">${dur} &middot; ${meta.segment_count} segments</div>
            </div>
            <div class="session-item-actions">
                <button class="sm-btn session-open-btn" title="Open">&#128065;</button>
                <button class="sm-btn session-export-btn" title="Export .md">&#8659;</button>
                <button class="sm-btn session-delete-btn" title="Delete">&#x1F5D1;</button>
            </div>
        `;

        el.querySelector('.session-open-btn').addEventListener('click', e => {
            e.stopPropagation();
            this._openSession(meta);
        });
        el.querySelector('.session-export-btn').addEventListener('click', e => {
            e.stopPropagation();
            this._exportSession(meta.timestamp);
        });
        el.querySelector('.session-delete-btn').addEventListener('click', e => {
            e.stopPropagation();
            this._deleteSession(meta.timestamp, el);
        });
        el.addEventListener('click', () => this._openSession(meta));

        return el;
    }

    async _openSession(meta) {
        try {
            const segments = await window.__TAURI__.core.invoke('load_session', { timestamp: meta.timestamp });
            this._viewingTimestamp = meta.timestamp;

            this.transcriptRenderer.clear();
            segments.forEach(seg => this.transcriptRenderer.appendSegment(seg));

            const date = new Date(meta.timestamp * 1000);
            this.bannerLabel.textContent = `Viewing session from ${date.toLocaleString()}`;
            this.banner.classList.remove('hidden');

            this.closeSidebar();
        } catch (err) {
            this.errorUI.show(`Could not open session: ${err}`);
        }
    }

    async _exportSession(timestamp) {
        try {
            await window.__TAURI__.core.invoke('export_session_to_file', { timestamp });
        } catch (err) {
            this.errorUI.show(`Export failed: ${err}`);
        }
    }

    async _deleteSession(timestamp, el) {
        try {
            await window.__TAURI__.core.invoke('delete_session', { timestamp });
            el.remove();
            if (this._viewingTimestamp === timestamp) {
                this.returnToLive();
            }
            if (this.sessionList.children.length === 0) {
                this.sessionList.innerHTML = '<div class="session-empty">No past sessions yet</div>';
            }
        } catch (err) {
            this.errorUI.show(`Delete failed: ${err}`);
        }
    }

    // Save the current live segments before switching to a past session view.
    saveLiveSegments(segments) {
        this._liveSegments = segments;
    }

    returnToLive() {
        this._viewingTimestamp = null;
        this.banner.classList.add('hidden');
        this.transcriptRenderer.clear();
        this._liveSegments.forEach(seg => this.transcriptRenderer.appendSegment(seg));
    }

    _fmtDuration(secs) {
        const m = Math.floor(secs / 60);
        const s = secs % 60;
        return `${m}:${String(s).padStart(2, '0')}`;
    }
}
