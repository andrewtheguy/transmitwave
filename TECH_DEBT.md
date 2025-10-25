# Technical Debt

This document tracks known technical debt and areas for improvement in the transmitwave project.

## 1. Detection Threshold Inconsistency (Web UI vs Backend Decoder)

**Severity:** Medium
**Component:** Web UI (`/preamble-postamble-record`) and Backend Decoder
**Status:** Active Tech Debt
**Date Identified:** 2025-10-25

### Problem

The web UI allows users to set a custom detection threshold for preamble/postamble signals (0.1-0.9), but the backend decoder re-detects these signals with hardcoded thresholds instead of:
1. Skipping detection for already-captured audio clips
2. Accepting pre-detected boundaries as input parameters
3. Using the same threshold configuration as the UI

### Impact

When a user sets a custom threshold to detect preamble/postamble during recording, the backend decoder may use a different threshold during decoding, causing:
- Inconsistent detection results
- Failed decoding despite successful detection during recording
- Threshold mismatches between web UI and CLI/backend tools

### Example Scenario

1. User sets threshold to 0.2 (more sensitive) in web UI
2. Preamble is detected with threshold 0.2 during recording
3. Audio is captured with preamble boundaries determined at 0.2 threshold
4. Backend decoder re-detects with hardcoded threshold (e.g., 0.4)
5. Decoder may fail or produce different boundaries

### Solution (Future Enhancement)

**Option A: Skip Re-detection**
- Modify backend decoder to accept audio clips without preamble/postamble on the clip
- Decode the content without re-detecting boundaries
- It works for current approach because preamble/postamble are just guide markers for the decoder to find the data section but they are not needed for the actual decoding process.

**Option B: Skip Re-detection**
- Backend should accept a `--no-detect-boundaries` flag or detect mode parameter
- For pre-recorded audio clips, skip preamble/postamble detection
- Decode using the audio as-is

**Option C: Accept Boundary Parameters**
- API should accept pre-detected preamble/postamble positions
- Backend uses provided boundaries instead of re-detecting
- Ensures consistency with UI threshold settings

**Option D: Threshold Configuration**
- Allow threshold to be passed through the API/CLI
- Backend uses user-specified threshold instead of hardcoded values
- Ensures consistent behavior between UI and backend

### Files Involved

- Web UI: `/web/src/pages/PreamblePostambleRecordPage.tsx`
  - Detection threshold setting: lines 537-543
  - Decode function: lines 351-443
- Backend Decoder: WASM module (transmitwave-wasm)
- Note: This warning is visible to users at the top of the Auto-Record page

---

## Notes

- Consider adding threshold parameter validation
- Document threshold behavior clearly in API documentation
