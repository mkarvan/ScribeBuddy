import { TranscriptRenderer } from './transcript.js';
import { Controls, DownloadUI, ErrorUI } from './controls.js';
import { Settings } from './settings.js';

document.addEventListener('DOMContentLoaded', async () => {
    const transcript = new TranscriptRenderer('#transcript');
    const controls = new Controls();
    const downloadUI = new DownloadUI();
    const errorUI = new ErrorUI();
    const settings = new Settings();

    await settings.loadApps();
    await settings.checkModel();
    settings.bind();
    errorUI.bind();

    // Restore persisted settings from backend
    try {
        const savedConfig = await window.__TAURI__.core.invoke('get_config');
        settings.loadFromConfig(savedConfig);
        // Apply saved app selection now that the <select> is populated
        settings.applyPendingApp();
    } catch (err) {
        console.warn('Could not restore settings:', err);
    }

    // --- Model download events ---
    window.__TAURI__.event.listen('model-download-started', () => {
        downloadUI.show();
    });

    window.__TAURI__.event.listen('model-download-progress', (event) => {
        const { downloaded, total, done } = event.payload;
        downloadUI.update(downloaded, total, done);
    });

    window.__TAURI__.event.listen('model-download-error', (event) => {
        errorUI.show(`Download failed: ${event.payload}`);
        downloadUI.hide();
    });

    // --- Session state events ---
    window.__TAURI__.event.listen('session-state-changed', (event) => {
        const state = event.payload;
        controls.updateButtons(state);

        switch (state) {
            case 'Recording':
                controls.startTimer();
                break;
            case 'Paused':
                controls.pauseTimer();
                break;
            case 'Stopped':
                controls.stopTimer();
                break;
            default:
                controls.resetTimer();
                break;
        }
    });

    // --- Processor / session errors ---
    window.__TAURI__.event.listen('session-error', (event) => {
        errorUI.show(event.payload, 8000);
    });

    // --- Live transcript segments ---
    window.__TAURI__.event.listen('transcript-segment', (event) => {
        transcript.appendSegment(event.payload);
        controls.updateSegmentCount(
            document.querySelectorAll('.transcript-segment').length
        );
    });

    // --- Control buttons ---
    document.getElementById('start-btn').addEventListener('click', async () => {
        try {
            transcript.clear();
            await window.__TAURI__.core.invoke('start_session');
        } catch (err) {
            errorUI.show(`Failed to start: ${err}`);
        }
    });

    document.getElementById('pause-btn').addEventListener('click', async () => {
        try {
            await window.__TAURI__.core.invoke('pause_session');
        } catch (err) {
            errorUI.show(`Failed to pause: ${err}`);
        }
    });

    document.getElementById('resume-btn').addEventListener('click', async () => {
        try {
            await window.__TAURI__.core.invoke('resume_session');
        } catch (err) {
            errorUI.show(`Failed to resume: ${err}`);
        }
    });

    document.getElementById('stop-btn').addEventListener('click', async () => {
        try {
            await window.__TAURI__.core.invoke('stop_session');
        } catch (err) {
            errorUI.show(`Failed to stop: ${err}`);
        }
    });

    document.getElementById('export-btn').addEventListener('click', async () => {
        try {
            await window.__TAURI__.core.invoke('export_markdown_to_file');
        } catch (err) {
            errorUI.show(`Export failed: ${err}`);
        }
    });

    // --- Poll fallback for segments (in case event channel is unavailable) ---
    let pollInterval = null;
    let lastSegmentCount = 0;

    const pollSegments = async () => {
        try {
            const segments = await window.__TAURI__.core.invoke('get_transcript');
            if (segments.length > lastSegmentCount) {
                for (let i = lastSegmentCount; i < segments.length; i++) {
                    transcript.appendSegment(segments[i]);
                }
                controls.updateSegmentCount(segments.length);
                lastSegmentCount = segments.length;
            }
        } catch (_) {}
    };

    window.__TAURI__.event.listen('session-state-changed', (event) => {
        const state = event.payload;
        if (state === 'Recording') {
            pollInterval = setInterval(pollSegments, 500);
        } else if (state === 'Idle' || state === 'Stopped') {
            clearInterval(pollInterval);
        }
    });

    controls.updateButtons('Idle');
});
