export class Controls {
    constructor() {
        this.startBtn = document.getElementById('start-btn');
        this.pauseBtn = document.getElementById('pause-btn');
        this.resumeBtn = document.getElementById('resume-btn');
        this.stopBtn = document.getElementById('stop-btn');
        this.exportBtn = document.getElementById('export-btn');
        this.statusBadge = document.getElementById('status-badge');
        this.timerEl = document.getElementById('timer');
        this.segmentCountEl = document.getElementById('segment-count');

        this.timerInterval = null;
        this.timerStart = null;
        this.pausedElapsed = 0;
    }

    updateButtons(state) {
        const isRecording = state === 'Recording';
        const isPaused = state === 'Paused';
        const isStopped = state === 'Stopped';

        this.startBtn.disabled = isRecording || isPaused;
        this.pauseBtn.disabled = !isRecording;
        this.resumeBtn.disabled = !isPaused;
        this.stopBtn.disabled = !isRecording && !isPaused;
        this.exportBtn.disabled = !isStopped;

        this.statusBadge.textContent = state;
        this.statusBadge.className = state.toLowerCase();
    }

    startTimer() {
        this.timerStart = Date.now();
        this.pausedElapsed = 0;
        this.updateTimer();
        this.timerInterval = setInterval(() => this.updateTimer(), 1000);
    }

    pauseTimer() {
        if (this.timerStart) {
            this.pausedElapsed += Date.now() - this.timerStart;
        }
        clearInterval(this.timerInterval);
        this.timerInterval = null;
    }

    resumeTimer() {
        this.timerStart = Date.now();
        this.updateTimer();
        this.timerInterval = setInterval(() => this.updateTimer(), 1000);
    }

    stopTimer() {
        clearInterval(this.timerInterval);
        this.timerInterval = null;
    }

    resetTimer() {
        clearInterval(this.timerInterval);
        this.timerInterval = null;
        this.timerStart = null;
        this.pausedElapsed = 0;
        this.timerEl.textContent = '00:00:00';
    }

    updateTimer() {
        if (!this.timerStart) return;
        const elapsed = this.pausedElapsed + (Date.now() - this.timerStart);
        const totalSecs = Math.floor(elapsed / 1000);
        const h = Math.floor(totalSecs / 3600);
        const m = Math.floor((totalSecs % 3600) / 60);
        const s = totalSecs % 60;
        this.timerEl.textContent =
            `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
    }

    updateSegmentCount(count) {
        this.segmentCountEl.textContent = `${count} segments`;
    }
}

export class DownloadUI {
    constructor() {
        this.bar = document.getElementById('download-bar');
        this.label = document.getElementById('download-label');
        this.progressText = document.getElementById('download-progress');
        this.fill = document.getElementById('progress-fill');
    }

    show() {
        this.bar.classList.remove('hidden');
        this.fill.style.width = '0%';
        this.progressText.textContent = '0%';
    }

    hide() {
        this.bar.classList.add('hidden');
    }

    update(downloaded, total, done) {
        if (done) {
            this.label.textContent = 'Download complete';
            this.fill.style.width = '100%';
            this.progressText.textContent = '100%';
            setTimeout(() => this.hide(), 2000);
            return;
        }
        const pct = total > 0 ? Math.round((downloaded / total) * 100) : 0;
        const mb = (downloaded / 1_048_576).toFixed(1);
        const mbTotal = total > 0 ? (total / 1_048_576).toFixed(1) : '?';
        this.label.textContent = `${mb}/${mbTotal} MB`;
        this.fill.style.width = `${pct}%`;
        this.progressText.textContent = `${pct}%`;
    }
}

export class ErrorUI {
    constructor() {
        this.bar = document.getElementById('error-bar');
        this.text = document.getElementById('error-text');
        this.dismissBtn = document.getElementById('error-dismiss');
    }

    show(msg, autoHideMs = 5000) {
        this.text.textContent = msg;
        this.bar.classList.remove('hidden');
        if (autoHideMs > 0) {
            setTimeout(() => this.hide(), autoHideMs);
        }
    }

    hide() {
        this.bar.classList.add('hidden');
    }

    bind() {
        this.dismissBtn.addEventListener('click', () => this.hide());
    }
}
