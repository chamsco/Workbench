Familiars on core: before and after.

Before is `claude/project-thread-gg3ogp` at 35b03d7. After is this branch. Both are the real GPUI window: a Linux debug build under Xvfb (lavapipe Vulkan), 1200x720, dark and light themes, an empty data folder, and `CAULDRON_FIXTURE_FAMILIARS=1` for the made-up familiars, which are marked Preview. No model was contacted.

- `new-*`: the New stream box. The stage bar now shows Plan, Execute, Verify, Ship and Learn. The Familiar toggle sits next to Plan.
- `new-familiar-*`: Familiar turned on. Before, the click hits nothing. After, it adds "Ship as" and a note that a familiar needs a folder.
- `familiar-*`: a familiar waiting on you.
- `familiar-done-*`: a delivered familiar. It shows the delivery card with each criterion, an Open pull request button and no merge.

On both sides, the Highlights "Budget" tab sits under the window controls. That isn't from this change.
