import { TranscriptRenderer } from './transcript.js';
import { Controls, DownloadUI, ErrorUI } from './controls.js';
import { Settings } from './settings.js';
import { SessionHistory } from './sessions.js';

document.addEventListener('DOMContentLoaded', async () => {
    const transcript = new TranscriptRenderer('#transcript');
    const controls = new Controls();
    const downloadUI = new DownloadUI();
    const errorUI = new ErrorUI();
    const settings = new Settings();
    const sessionHistory = new SessionHistory(transcript, errorUI);

    await settings.loadApps();
    await settings.checkModel();
    settings.bind();
    errorUI.bind();

    // Restore persisted settings from backend
    try {
        const savedConfig = await window.__TAURI__.core.invoke('get_config');
        settings.loadFromConfig(savedConfig);
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
                // Refresh history sidebar so the new session appears.
                if (!sessionHistory.sidebar.classList.contains('closed')) {
                    sessionHistory.refresh();
                }
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
        // Only append to live view; ignore while user is viewing a past session.
        if (!sessionHistory.isViewing()) {
            transcript.appendSegment(event.payload);
            controls.updateSegmentCount(
                document.querySelectorAll('.transcript-segment').length
            );
        }
    });

    // --- Control buttons ---
    document.getElementById('start-btn').addEventListener('click', async () => {
        try {
            // If viewing a past session, return to live first.
            if (sessionHistory.isViewing()) sessionHistory.returnToLive();
            transcript.clear();
            sessionHistory.saveLiveSegments([]);
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
            // Snapshot live segments before stopping so returnToLive() can restore them.
            sessionHistory.saveLiveSegments(
                Array.from(document.querySelectorAll('.transcript-segment')).map(el => ({
                    speaker: el.classList.contains('you') ? 'You' : 'Remote',
                    text: el.querySelector('.segment-text')?.textContent ?? '',
                    start_time: 0,
                    end_time: 0,
                }))
            );
            await window.__TAURI__.core.invoke('stop_session');
        } catch (err) {
            errorUI.show(`Failed to stop: ${err}`);
        }
    });

    document.getElementById('export-btn').addEventListener('click', async () => {
        try {
            // Export the currently-viewed session if browsing history, else current live session.
            if (sessionHistory.isViewing()) {
                await window.__TAURI__.core.invoke('export_session_to_file', {
                    timestamp: sessionHistory._viewingTimestamp,
                });
            } else {
                await window.__TAURI__.core.invoke('export_markdown_to_file');
            }
        } catch (err) {
            errorUI.show(`Export failed: ${err}`);
        }
    });

    controls.updateButtons('Idle');
});
