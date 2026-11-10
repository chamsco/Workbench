# open-bot-avatar

Open Bot Avatar renders object avatars from an SVG silhouette and eye placement. The renderer adds volume, blinking, gaze and motion.

The `open-bot-avatar` package has no dependencies. Its main entry point works without a DOM; `open-bot-avatar/dom` adds browser mounting and pointer tracking.

## Install

```sh
pnpm add open-bot-avatar
```

## Mount an avatar

Create an empty element with an explicit size, such as a 96 × 96 pixel div with `id="bot"`. `mountAvatar` fills that element.

```ts
import { getExample } from "open-bot-avatar";
import { mountAvatar } from "open-bot-avatar/dom";

const avatar = mountAvatar(document.querySelector("#bot")!, {
  definition: getExample("mug")!,
  color: "#ff6a1a",
  followPointer: true,
});

avatar.update({ mood: "happy" });
// When the avatar is no longer needed:
avatar.destroy();
```

`update` accepts any subset of the mount options. Omitted options keep their values. Call `destroy` when the avatar is no longer needed; it stops tracking and animation and removes the avatar's content.

| Option | Behavior |
| --- | --- |
| `definition` | Required avatar definition. Validate imported data first. |
| `color`, `eyeColor` | Body and eye colors as `#rgb` or `#rrggbb`. Defaults: `#ff6a1a`, `#151515`. |
| `mood` | `"idle"` (default), `"thinking"` or `"happy"`. Changes ease from the current pose. |
| `followPointer` | Follow the mouse, pen or finger on the page. Default: `false`. |
| `lookAt` | Fixed `{ x, y }` direction, each from -1 to 1 (x right, y down). Overrides the pointer; `null` clears it. |
| `holdGaze` | Keep the current look direction. Default: `false`. |
| `view` | Extra `{ yaw, pitch }` rotation in radians, relative to the resting view. |
| `depth` | Volume amount from 0 to 14. Default: 6; 0 is flat. |
| `reducedMotion` | `true`, `false` or `"system"` (default), which follows `prefers-reduced-motion`. |
| `still` | Render a static SVG without animation. Default: `false`. |
| `label` | Accessible name. Defaults to the definition's name; `""` marks it decorative. |

## React

There is no separate React package. Copy this component to mount on setup, update after rendering, and destroy on cleanup. In other frameworks, use the equivalent lifecycle hooks.

```tsx
import { useEffect, useRef } from "react";
import { mountAvatar, type AvatarOptions, type MountedAvatar } from "open-bot-avatar/dom";

export function BotAvatar({ size = 96, ...options }: AvatarOptions & { size?: number }) {
  const host = useRef<HTMLDivElement>(null);
  const avatar = useRef<MountedAvatar | null>(null);
  useEffect(() => {
    avatar.current = mountAvatar(host.current!, options);
    return () => avatar.current?.destroy();
  }, []);
  useEffect(() => avatar.current?.update(options));
  return <div ref={host} style={{ width: size, aspectRatio: "1" }} />;
}

// Pass a validated definition as the definition prop.
```

## Static SVG and frames

`renderAvatarSvg` returns a standalone SVG string. `avatarFrame` returns paths, transforms and eye shapes in back-to-front drawing order for custom renderers. Neither needs a DOM.

```ts
import { avatarFrame, renderAvatarSvg, validateAvatar } from "open-bot-avatar";

// text is a JSON string. JSON.parse throws if its syntax is invalid.
const result = validateAvatar(JSON.parse(text));
if (!result.ok) throw new Error(result.errors.join("\n"));

const options = { color: "#1fbf68", time: 1.2, motion: "happy" as const };
const svg = renderAvatarSvg(result.value, { ...options, size: 256 });
const frame = avatarFrame(result.value, options);
```

Both functions accept `RenderOptions`:

- `color`, `eyeColor`, `depth` and `view` work as in the mount API.
- `motion` selects the mood (default: `"idle"`); `time` sets the animation clock in seconds (default: 0).
- `gaze` sets a fixed direction: `{ x, y, weight? }`. Each axis ranges from -1 to 1; `weight` ranges from 0 to 1 and defaults to 1.
- `size` sets the SVG width and height in pixels. Omit it for a fluid SVG.
- `reducedMotion` is a boolean, defaulting to `false`. It freezes body motion and blinking; gaze still applies. It does not read system preferences.
- `motionBlend` lets a custom animation host blend from another mood: `{ from, t }`, with `t` from 0 to 1. `from` can be a mood or `{ motion, blend? }` for an interrupted transition.

## Definitions and validation

A definition contains one filled SVG path in a `0 0 100 100` viewBox and an eye pair. The path can have multiple closed subpaths; holes wind opposite to their outline. Color and animation are renderer options.

```json
{
  "version": 1,
  "id": "box",
  "name": "Box",
  "body": { "path": "M20 20 H80 V80 H20 Z" },
  "eyes": { "x": 50, "y": 46, "spacing": 15, "scale": 1 }
}
```

`x` and `y` locate the center of the eye pair. `spacing` is the center-to-center distance; `scale` adjusts eye size. Use a lowercase slug for `id` and a short display name for `name`. Unknown fields are rejected.

Parse JSON text first, then call `validateAvatar` before rendering imported definitions. It returns `{ ok: true, value }` with a normalized definition or `{ ok: false, errors }` with validation errors. Invalid JSON syntax throws in `JSON.parse`, before validation.

## Make an avatar

In the [editor](https://dominikscholz.github.io/open-bot-avatar/), describe an avatar in Generate. Copy the prompt into a chat model or open it in an agent app. The editor does not run a model. Paste the returned JSON into Import or drop a JSON file. Import validates it before adding it. If validation fails, use the repair prompt to request a correction.

See the [documentation](https://dominikscholz.github.io/open-bot-avatar/#/docs) and [definition contract](https://github.com/dominikscholz/open-bot-avatar/blob/main/docs/avatar-contract.md).

## Other exports

- `examples` and `getExample(id)` provide built-in definitions.
- `renderAvatarScene` and `sceneFrame` provide CSS 3D scene markup and per-frame transforms.
- `computePose`, `createAnimator` and `pointerGaze` support custom animation hosts.

## License

MIT
