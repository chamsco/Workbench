/** Minimal hex color handling for the renderer. Only #rgb / #rrggbb are accepted. */
export type Rgb = [number, number, number];
export declare function parseHex(input: unknown): Rgb | null;
export declare function toHex([r, g, b]: Rgb): string;
export declare function mix(a: Rgb, b: Rgb, t: number): Rgb;
/**
 * Darken while keeping the hue rich: blend toward a deeper, slightly
 * saturated version of the color rather than toward gray-black.
 */
export declare function shade(c: Rgb, t: number): Rgb;
export declare function tint(c: Rgb, t: number): Rgb;
/** WCAG relative luminance. */
export declare function luminance([r, g, b]: Rgb): number;
export declare function contrast(a: Rgb, b: Rgb): number;
export declare const DARK_INK: Rgb;
export declare const LIGHT_INK: Rgb;
/** Pick the eye ink with the best contrast against the body color. */
export declare function autoEyeColor(body: Rgb): Rgb;
//# sourceMappingURL=color.d.ts.map