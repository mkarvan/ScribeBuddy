export class TranscriptRenderer {
    constructor(containerSelector) {
        this.container = document.querySelector(containerSelector);
        this.autoScroll = true;
        this.setupScrollDetection();
    }

    setupScrollDetection() {
        const parent = this.container.parentElement;
        parent.addEventListener('scroll', () => {
            const threshold = 50;
            const atBottom =
                parent.scrollHeight - parent.scrollTop - parent.clientHeight < threshold;
            this.autoScroll = atBottom;
        });
    }

    clear() {
        this.container.innerHTML = '';
    }

    appendSegment(segment) {
        const placeholder = this.container.querySelector('.placeholder');
        if (placeholder) placeholder.remove();

        const div = document.createElement('div');
        div.className = `transcript-segment ${segment.speaker.toLowerCase()}`;
        const time = this.formatDuration(segment.start_time);

        div.innerHTML = `
            <div class="segment-header">
                <span class="segment-speaker ${segment.speaker.toLowerCase()}">
                    ${segment.speaker}
                </span>
                <span class="segment-time">${time}</span>
            </div>
            <div class="segment-text">${this.escapeHtml(segment.text.trim())}</div>
        `;

        this.container.appendChild(div);
        this.scrollToBottom();
    }

    scrollToBottom() {
        if (this.autoScroll) {
            const parent = this.container.parentElement;
            parent.scrollTop = parent.scrollHeight;
        }
    }

    formatDuration(dur) {
        let secs = 0;
        if (typeof dur === 'object' && dur.secs !== undefined) {
            secs = dur.secs;
        } else if (typeof dur === 'number') {
            secs = dur;
        }
        const h = Math.floor(secs / 3600);
        const m = Math.floor((secs % 3600) / 60);
        const s = secs % 60;
        return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(Math.floor(s)).padStart(2, '0')}`;
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }
}
