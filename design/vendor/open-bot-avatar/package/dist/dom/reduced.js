/** The system's prefers-reduced-motion setting, watched once for the whole page. */
import { requestFrame } from './clock.js';
const QUERY = '(prefers-reduced-motion: reduce)';
let mql;
function media() {
    if (mql === undefined) {
        mql = typeof matchMedia === 'undefined' ? null : matchMedia(QUERY);
        // Live avatars pick up a change on their next frame; wake the loop so they get one.
        mql?.addEventListener('change', requestFrame);
    }
    return mql;
}
export function prefersReducedMotion() {
    return media()?.matches ?? false;
}
//# sourceMappingURL=reduced.js.map