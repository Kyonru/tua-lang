// Generated from love2d-community/love-api 11.5.
// Source data: https://github.com/love2d-community/love-api
// Do not edit entries by hand; regenerate from love_api.lua when bumping LOVE API data.

use crate::lua::{LuaItem, LuaItemKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LoveApiItem {
    path: &'static [&'static str],
    label: &'static str,
    kind: LuaItemKind,
    detail: &'static str,
    documentation: &'static str,
}

pub(crate) fn field_items(receiver_parts: &[String], prefix: &str) -> Vec<LuaItem> {
    ITEMS
        .iter()
        .filter(|item| path_matches(item.path, receiver_parts))
        .filter(|item| item.label.starts_with(prefix))
        .map(lua_item)
        .collect()
}

pub(crate) fn hover_item(parts: &[String]) -> Option<LuaItem> {
    let (label, receiver) = parts.split_last()?;

    ITEMS
        .iter()
        .find(|item| path_matches(item.path, receiver) && item.label == label)
        .map(lua_item)
}

fn path_matches(path: &[&str], receiver_parts: &[String]) -> bool {
    path.len() == receiver_parts.len()
        && path
            .iter()
            .zip(receiver_parts)
            .all(|(left, right)| *left == right)
}

fn lua_item(item: &LoveApiItem) -> LuaItem {
    LuaItem {
        label: item.label,
        kind: item.kind,
        detail: item.detail,
        documentation: item.documentation,
    }
}

const ITEMS: &[LoveApiItem] = &[
    LoveApiItem {
        path: &["love"],
        label: "audio",
        kind: LuaItemKind::Module,
        detail: r"LÖVE audio module",
        documentation: r"Provides an interface to create noise with the user's speakers.",
    },
    LoveApiItem {
        path: &["love"],
        label: "conf",
        kind: LuaItemKind::Function,
        detail: r"function love.conf(t: table)",
        documentation: r"If a file called conf.lua is present in your game folder (or .love file), it is run before the LÖVE modules are loaded. You can use this file to overwrite the love.conf function, which is later called by the LÖVE 'boot' script. Using the love.conf function, you can set some configuration options, and change things like the default size of the window, which modules are loaded, and other stuff.",
    },
    LoveApiItem {
        path: &["love"],
        label: "data",
        kind: LuaItemKind::Module,
        detail: r"LÖVE data module",
        documentation: r"Provides functionality for creating and transforming data.",
    },
    LoveApiItem {
        path: &["love"],
        label: "directorydropped",
        kind: LuaItemKind::Function,
        detail: r"function love.directorydropped(path: string)",
        documentation: r"Callback function triggered when a directory is dragged and dropped onto the window.",
    },
    LoveApiItem {
        path: &["love"],
        label: "displayrotated",
        kind: LuaItemKind::Function,
        detail: r"function love.displayrotated(index: number, orientation: DisplayOrientation)",
        documentation: r"Called when the device display orientation changed, for example, user rotated their phone 180 degrees.",
    },
    LoveApiItem {
        path: &["love"],
        label: "draw",
        kind: LuaItemKind::Function,
        detail: r"function love.draw()",
        documentation: r"Callback function used to draw on the screen every frame.",
    },
    LoveApiItem {
        path: &["love"],
        label: "errorhandler",
        kind: LuaItemKind::Function,
        detail: r"function love.errorhandler(msg: string): function",
        documentation: r"The error handler, used to display error messages.",
    },
    LoveApiItem {
        path: &["love"],
        label: "event",
        kind: LuaItemKind::Module,
        detail: r"LÖVE event module",
        documentation: r"Manages events, like keypresses.",
    },
    LoveApiItem {
        path: &["love"],
        label: "filedropped",
        kind: LuaItemKind::Function,
        detail: r"function love.filedropped(file: DroppedFile)",
        documentation: r"Callback function triggered when a file is dragged and dropped onto the window.",
    },
    LoveApiItem {
        path: &["love"],
        label: "filesystem",
        kind: LuaItemKind::Module,
        detail: r"LÖVE filesystem module",
        documentation: r"Provides an interface to the user's filesystem.",
    },
    LoveApiItem {
        path: &["love"],
        label: "focus",
        kind: LuaItemKind::Function,
        detail: r"function love.focus(focus: boolean)",
        documentation: r"Callback function triggered when window receives or loses focus.",
    },
    LoveApiItem {
        path: &["love"],
        label: "font",
        kind: LuaItemKind::Module,
        detail: r"LÖVE font module",
        documentation: r"Allows you to work with fonts.",
    },
    LoveApiItem {
        path: &["love"],
        label: "gamepadaxis",
        kind: LuaItemKind::Function,
        detail: r"function love.gamepadaxis(joystick: Joystick, axis: GamepadAxis, value: number)",
        documentation: r"Called when a Joystick's virtual gamepad axis is moved.",
    },
    LoveApiItem {
        path: &["love"],
        label: "gamepadpressed",
        kind: LuaItemKind::Function,
        detail: r"function love.gamepadpressed(joystick: Joystick, button: GamepadButton)",
        documentation: r"Called when a Joystick's virtual gamepad button is pressed.",
    },
    LoveApiItem {
        path: &["love"],
        label: "gamepadreleased",
        kind: LuaItemKind::Function,
        detail: r"function love.gamepadreleased(joystick: Joystick, button: GamepadButton)",
        documentation: r"Called when a Joystick's virtual gamepad button is released.",
    },
    LoveApiItem {
        path: &["love"],
        label: "getVersion",
        kind: LuaItemKind::Function,
        detail: r"function love.getVersion(): number, number, number, string",
        documentation: r"Gets the current running version of LÖVE.",
    },
    LoveApiItem {
        path: &["love"],
        label: "graphics",
        kind: LuaItemKind::Module,
        detail: r"LÖVE graphics module",
        documentation: r"The primary responsibility for the love.graphics module is the drawing of lines, shapes, text, Images and other Drawable objects onto the screen. Its secondary responsibilities include loading external files (including Images and Fonts) into memory, creating specialized objects (such as ParticleSystems or Canvases) and managing screen geometry.

LÖVE's coordinate system is rooted in the upper-left corner of the screen, which is at location (0, 0). The x axis is horizontal: larger values are further to the right. The y axis is vertical: larger values are further towards the bottom.

In many cases, you draw images or shapes in terms of their upper-left corner.

Many of the functions are used to manipulate the graphics coordinate system, which is essentially the way coordinates are mapped to the display. You can change the position, scale, and even rotation in this way.",
    },
    LoveApiItem {
        path: &["love"],
        label: "hasDeprecationOutput",
        kind: LuaItemKind::Function,
        detail: r"function love.hasDeprecationOutput(): boolean",
        documentation: r"Gets whether LÖVE displays warnings when using deprecated functionality. It is disabled by default in fused mode, and enabled by default otherwise.

When deprecation output is enabled, the first use of a formally deprecated LÖVE API will show a message at the bottom of the screen for a short time, and print the message to the console.",
    },
    LoveApiItem {
        path: &["love"],
        label: "image",
        kind: LuaItemKind::Module,
        detail: r"LÖVE image module",
        documentation: r"Provides an interface to decode encoded image data.",
    },
    LoveApiItem {
        path: &["love"],
        label: "isVersionCompatible",
        kind: LuaItemKind::Function,
        detail: r"function love.isVersionCompatible(version: string): boolean",
        documentation: r"Gets whether the given version is compatible with the current running version of LÖVE.",
    },
    LoveApiItem {
        path: &["love"],
        label: "joystick",
        kind: LuaItemKind::Module,
        detail: r"LÖVE joystick module",
        documentation: r"Provides an interface to the user's joystick.",
    },
    LoveApiItem {
        path: &["love"],
        label: "joystickadded",
        kind: LuaItemKind::Function,
        detail: r"function love.joystickadded(joystick: Joystick)",
        documentation: r"Called when a Joystick is connected.",
    },
    LoveApiItem {
        path: &["love"],
        label: "joystickaxis",
        kind: LuaItemKind::Function,
        detail: r"function love.joystickaxis(joystick: Joystick, axis: number, value: number)",
        documentation: r"Called when a joystick axis moves.",
    },
    LoveApiItem {
        path: &["love"],
        label: "joystickhat",
        kind: LuaItemKind::Function,
        detail: r"function love.joystickhat(joystick: Joystick, hat: number, direction: JoystickHat)",
        documentation: r"Called when a joystick hat direction changes.",
    },
    LoveApiItem {
        path: &["love"],
        label: "joystickpressed",
        kind: LuaItemKind::Function,
        detail: r"function love.joystickpressed(joystick: Joystick, button: number)",
        documentation: r"Called when a joystick button is pressed.",
    },
    LoveApiItem {
        path: &["love"],
        label: "joystickreleased",
        kind: LuaItemKind::Function,
        detail: r"function love.joystickreleased(joystick: Joystick, button: number)",
        documentation: r"Called when a joystick button is released.",
    },
    LoveApiItem {
        path: &["love"],
        label: "joystickremoved",
        kind: LuaItemKind::Function,
        detail: r"function love.joystickremoved(joystick: Joystick)",
        documentation: r"Called when a Joystick is disconnected.",
    },
    LoveApiItem {
        path: &["love"],
        label: "keyboard",
        kind: LuaItemKind::Module,
        detail: r"LÖVE keyboard module",
        documentation: r"Provides an interface to the user's keyboard.",
    },
    LoveApiItem {
        path: &["love"],
        label: "keypressed",
        kind: LuaItemKind::Function,
        detail: r"function love.keypressed(key: KeyConstant, scancode: Scancode, isrepeat: boolean)",
        documentation: r"Callback function triggered when a key is pressed.",
    },
    LoveApiItem {
        path: &["love"],
        label: "keyreleased",
        kind: LuaItemKind::Function,
        detail: r"function love.keyreleased(key: KeyConstant, scancode: Scancode)",
        documentation: r"Callback function triggered when a keyboard key is released.",
    },
    LoveApiItem {
        path: &["love"],
        label: "load",
        kind: LuaItemKind::Function,
        detail: r"function love.load(arg: table, unfilteredArg: table)",
        documentation: r"This function is called exactly once at the beginning of the game.",
    },
    LoveApiItem {
        path: &["love"],
        label: "lowmemory",
        kind: LuaItemKind::Function,
        detail: r"function love.lowmemory()",
        documentation: r"Callback function triggered when the system is running out of memory on mobile devices.

Mobile operating systems may forcefully kill the game if it uses too much memory, so any non-critical resource should be removed if possible (by setting all variables referencing the resources to '''nil'''), when this event is triggered. Sounds and images in particular tend to use the most memory.",
    },
    LoveApiItem {
        path: &["love"],
        label: "math",
        kind: LuaItemKind::Module,
        detail: r"LÖVE math module",
        documentation: r"Provides system-independent mathematical functions.",
    },
    LoveApiItem {
        path: &["love"],
        label: "mouse",
        kind: LuaItemKind::Module,
        detail: r"LÖVE mouse module",
        documentation: r"Provides an interface to the user's mouse.",
    },
    LoveApiItem {
        path: &["love"],
        label: "mousefocus",
        kind: LuaItemKind::Function,
        detail: r"function love.mousefocus(focus: boolean)",
        documentation: r"Callback function triggered when window receives or loses mouse focus.",
    },
    LoveApiItem {
        path: &["love"],
        label: "mousemoved",
        kind: LuaItemKind::Function,
        detail: r"function love.mousemoved(x: number, y: number, dx: number, dy: number, istouch: boolean)",
        documentation: r"Callback function triggered when the mouse is moved.",
    },
    LoveApiItem {
        path: &["love"],
        label: "mousepressed",
        kind: LuaItemKind::Function,
        detail: r"function love.mousepressed(x: number, y: number, button: number, istouch: boolean, presses: number)",
        documentation: r"Callback function triggered when a mouse button is pressed.",
    },
    LoveApiItem {
        path: &["love"],
        label: "mousereleased",
        kind: LuaItemKind::Function,
        detail: r"function love.mousereleased(x: number, y: number, button: number, istouch: boolean, presses: number)",
        documentation: r"Callback function triggered when a mouse button is released.",
    },
    LoveApiItem {
        path: &["love"],
        label: "physics",
        kind: LuaItemKind::Module,
        detail: r"LÖVE physics module",
        documentation: r"Can simulate 2D rigid body physics in a realistic manner. This module is based on Box2D, and this API corresponds to the Box2D API as closely as possible.",
    },
    LoveApiItem {
        path: &["love"],
        label: "quit",
        kind: LuaItemKind::Function,
        detail: r"function love.quit(): boolean",
        documentation: r"Callback function triggered when the game is closed.",
    },
    LoveApiItem {
        path: &["love"],
        label: "resize",
        kind: LuaItemKind::Function,
        detail: r"function love.resize(w: number, h: number)",
        documentation: r"Called when the window is resized, for example if the user resizes the window, or if love.window.setMode is called with an unsupported width or height in fullscreen and the window chooses the closest appropriate size.",
    },
    LoveApiItem {
        path: &["love"],
        label: "run",
        kind: LuaItemKind::Function,
        detail: r"function love.run(): function",
        documentation: r"The main function, containing the main loop. A sensible default is used when left out.",
    },
    LoveApiItem {
        path: &["love"],
        label: "setDeprecationOutput",
        kind: LuaItemKind::Function,
        detail: r"function love.setDeprecationOutput(enable: boolean)",
        documentation: r"Sets whether LÖVE displays warnings when using deprecated functionality. It is disabled by default in fused mode, and enabled by default otherwise.

When deprecation output is enabled, the first use of a formally deprecated LÖVE API will show a message at the bottom of the screen for a short time, and print the message to the console.",
    },
    LoveApiItem {
        path: &["love"],
        label: "sound",
        kind: LuaItemKind::Module,
        detail: r"LÖVE sound module",
        documentation: r"This module is responsible for decoding sound files. It can't play the sounds, see love.audio for that.",
    },
    LoveApiItem {
        path: &["love"],
        label: "system",
        kind: LuaItemKind::Module,
        detail: r"LÖVE system module",
        documentation: r"Provides access to information about the user's system.",
    },
    LoveApiItem {
        path: &["love"],
        label: "textedited",
        kind: LuaItemKind::Function,
        detail: r"function love.textedited(text: string, start: number, length: number)",
        documentation: r"Called when the candidate text for an IME (Input Method Editor) has changed.

The candidate text is not the final text that the user will eventually choose. Use love.textinput for that.",
    },
    LoveApiItem {
        path: &["love"],
        label: "textinput",
        kind: LuaItemKind::Function,
        detail: r"function love.textinput(text: string)",
        documentation: r"Called when text has been entered by the user. For example if shift-2 is pressed on an American keyboard layout, the text '@' will be generated.",
    },
    LoveApiItem {
        path: &["love"],
        label: "thread",
        kind: LuaItemKind::Module,
        detail: r"LÖVE thread module",
        documentation: r"Allows you to work with threads.

Threads are separate Lua environments, running in parallel to the main code. As their code runs separately, they can be used to compute complex operations without adversely affecting the frame rate of the main thread. However, as they are separate environments, they cannot access the variables and functions of the main thread, and communication between threads is limited.

All LOVE objects (userdata) are shared among threads so you'll only have to send their references across threads. You may run into concurrency issues if you manipulate an object on multiple threads at the same time.

When a Thread is started, it only loads the love.thread module. Every other module has to be loaded with require.",
    },
    LoveApiItem {
        path: &["love"],
        label: "threaderror",
        kind: LuaItemKind::Function,
        detail: r"function love.threaderror(thread: Thread, errorstr: string)",
        documentation: r"Callback function triggered when a Thread encounters an error.",
    },
    LoveApiItem {
        path: &["love"],
        label: "timer",
        kind: LuaItemKind::Module,
        detail: r"LÖVE timer module",
        documentation: r"Provides an interface to the user's clock.",
    },
    LoveApiItem {
        path: &["love"],
        label: "touch",
        kind: LuaItemKind::Module,
        detail: r"LÖVE touch module",
        documentation: r"Provides an interface to touch-screen presses.",
    },
    LoveApiItem {
        path: &["love"],
        label: "touchmoved",
        kind: LuaItemKind::Function,
        detail: r"function love.touchmoved(id: light userdata, x: number, y: number, dx: number, dy: number, pressure: number)",
        documentation: r"Callback function triggered when a touch press moves inside the touch screen.",
    },
    LoveApiItem {
        path: &["love"],
        label: "touchpressed",
        kind: LuaItemKind::Function,
        detail: r"function love.touchpressed(id: light userdata, x: number, y: number, dx: number, dy: number, pressure: number)",
        documentation: r"Callback function triggered when the touch screen is touched.",
    },
    LoveApiItem {
        path: &["love"],
        label: "touchreleased",
        kind: LuaItemKind::Function,
        detail: r"function love.touchreleased(id: light userdata, x: number, y: number, dx: number, dy: number, pressure: number)",
        documentation: r"Callback function triggered when the touch screen stops being touched.",
    },
    LoveApiItem {
        path: &["love"],
        label: "update",
        kind: LuaItemKind::Function,
        detail: r"function love.update(dt: number)",
        documentation: r"Callback function used to update the state of the game every frame.",
    },
    LoveApiItem {
        path: &["love"],
        label: "video",
        kind: LuaItemKind::Module,
        detail: r"LÖVE video module",
        documentation: r"This module is responsible for decoding, controlling, and streaming video files.

It can't draw the videos, see love.graphics.newVideo and Video objects for that.",
    },
    LoveApiItem {
        path: &["love"],
        label: "visible",
        kind: LuaItemKind::Function,
        detail: r"function love.visible(visible: boolean)",
        documentation: r"Callback function triggered when window is minimized/hidden or unminimized by the user.",
    },
    LoveApiItem {
        path: &["love"],
        label: "wheelmoved",
        kind: LuaItemKind::Function,
        detail: r"function love.wheelmoved(x: number, y: number)",
        documentation: r"Callback function triggered when the mouse wheel is moved.",
    },
    LoveApiItem {
        path: &["love"],
        label: "window",
        kind: LuaItemKind::Module,
        detail: r"LÖVE window module",
        documentation: r"Provides an interface for modifying and retrieving information about the program's window.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getActiveEffects",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getActiveEffects(): table",
        documentation: r"Gets a list of the names of the currently enabled effects.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getActiveSourceCount",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getActiveSourceCount(): number",
        documentation: r"Gets the current number of simultaneously playing sources.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getDistanceModel",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getDistanceModel(): DistanceModel",
        documentation: r"Returns the distance attenuation model.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getDopplerScale",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getDopplerScale(): number",
        documentation: r"Gets the current global scale factor for velocity-based doppler effects.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getEffect",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getEffect(name: string): table",
        documentation: r"Gets the settings associated with an effect.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getMaxSceneEffects",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getMaxSceneEffects(): number",
        documentation: r"Gets the maximum number of active effects supported by the system.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getMaxSourceEffects",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getMaxSourceEffects(): number",
        documentation: r"Gets the maximum number of active Effects in a single Source object, that the system can support.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getOrientation",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getOrientation(): number, number, number, number, number, number",
        documentation: r"Returns the orientation of the listener.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getPosition",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getPosition(): number, number, number",
        documentation: r"Returns the position of the listener. Please note that positional audio only works for mono (i.e. non-stereo) sources.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getRecordingDevices",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getRecordingDevices(): table",
        documentation: r"Gets a list of RecordingDevices on the system.

The first device in the list is the user's default recording device. The list may be empty if there are no microphones connected to the system.

Audio recording is currently not supported on iOS.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getVelocity",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getVelocity(): number, number, number",
        documentation: r"Returns the velocity of the listener.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "getVolume",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.getVolume(): number",
        documentation: r"Returns the master volume.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "isEffectsSupported",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.isEffectsSupported(): boolean",
        documentation: r"Gets whether audio effects are supported in the system.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "newQueueableSource",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.newQueueableSource(samplerate: number, bitdepth: number, channels: number, buffercount?: number): Source",
        documentation: r"Creates a new Source usable for real-time generated sound playback with Source:queue.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "newSource",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.newSource(filename: string, type: SourceType): Source",
        documentation: r"Creates a new Source from a filepath, File, Decoder or SoundData.

Sources created from SoundData are always static.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "pause",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.pause(): table",
        documentation: r"Pauses specific or all currently played Sources.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "play",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.play(source: Source)",
        documentation: r"Plays the specified Source.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "setDistanceModel",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.setDistanceModel(model: DistanceModel)",
        documentation: r"Sets the distance attenuation model.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "setDopplerScale",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.setDopplerScale(scale: number)",
        documentation: r"Sets a global scale factor for velocity-based doppler effects. The default scale value is 1.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "setEffect",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.setEffect(name: string, settings: table): boolean",
        documentation: r"Defines an effect that can be applied to a Source.

Not all system supports audio effects. Use love.audio.isEffectsSupported to check.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "setMixWithSystem",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.setMixWithSystem(mix: boolean): boolean",
        documentation: r"Sets whether the system should mix the audio with the system's audio.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "setOrientation",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.setOrientation(fx, fy, fz: number, ux, uy, uz: number)",
        documentation: r"Sets the orientation of the listener.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "setPosition",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.setPosition(x: number, y: number, z: number)",
        documentation: r"Sets the position of the listener, which determines how sounds play.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "setVelocity",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.setVelocity(x: number, y: number, z: number)",
        documentation: r"Sets the velocity of the listener.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "setVolume",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.setVolume(volume: number)",
        documentation: r"Sets the master volume.",
    },
    LoveApiItem {
        path: &["love", "audio"],
        label: "stop",
        kind: LuaItemKind::Function,
        detail: r"function love.audio.stop()",
        documentation: r"Stops currently played sources.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "compress",
        kind: LuaItemKind::Function,
        detail: r"function love.data.compress(container: ContainerType, format: CompressedDataFormat, rawstring: string, level?: number): CompressedData or string",
        documentation: r"Compresses a string or data using a specific compression algorithm.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "decode",
        kind: LuaItemKind::Function,
        detail: r"function love.data.decode(container: ContainerType, format: EncodeFormat, sourceString: string): ByteData or string",
        documentation: r"Decode Data or a string from any of the EncodeFormats to Data or string.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "decompress",
        kind: LuaItemKind::Function,
        detail: r"function love.data.decompress(container: ContainerType, compressedData: CompressedData): Data or string",
        documentation: r"Decompresses a CompressedData or previously compressed string or Data object.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "encode",
        kind: LuaItemKind::Function,
        detail: r"function love.data.encode(container: ContainerType, format: EncodeFormat, sourceString: string, linelength?: number): ByteData or string",
        documentation: r"Encode Data or a string to a Data or string in one of the EncodeFormats.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "getPackedSize",
        kind: LuaItemKind::Function,
        detail: r"function love.data.getPackedSize(format: string): number",
        documentation: r"Gets the size in bytes that a given format used with love.data.pack will use.

This function behaves the same as Lua 5.3's string.packsize.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "hash",
        kind: LuaItemKind::Function,
        detail: r"function love.data.hash(hashFunction: HashFunction, string: string): string",
        documentation: r"Compute the message digest of a string using a specified hash algorithm.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "newByteData",
        kind: LuaItemKind::Function,
        detail: r"function love.data.newByteData(datastring: string): ByteData",
        documentation: r"Creates a new Data object containing arbitrary bytes.

Data:getPointer along with LuaJIT's FFI can be used to manipulate the contents of the ByteData object after it has been created.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "newDataView",
        kind: LuaItemKind::Function,
        detail: r"function love.data.newDataView(data: Data, offset: number, size: number): Data",
        documentation: r"Creates a new Data referencing a subsection of an existing Data object.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "pack",
        kind: LuaItemKind::Function,
        detail: r"function love.data.pack(container: ContainerType, format: string, v1: number or boolean or string, ...): Data or string",
        documentation: r"Packs (serializes) simple Lua values.

This function behaves the same as Lua 5.3's string.pack.",
    },
    LoveApiItem {
        path: &["love", "data"],
        label: "unpack",
        kind: LuaItemKind::Function,
        detail: r"function love.data.unpack(format: string, datastring: string, pos?: number): number or boolean or string, number or boolean or string, number",
        documentation: r"Unpacks (deserializes) a byte-string or Data into simple Lua values.

This function behaves the same as Lua 5.3's string.unpack.",
    },
    LoveApiItem {
        path: &["love", "event"],
        label: "clear",
        kind: LuaItemKind::Function,
        detail: r"function love.event.clear()",
        documentation: r"Clears the event queue.",
    },
    LoveApiItem {
        path: &["love", "event"],
        label: "poll",
        kind: LuaItemKind::Function,
        detail: r"function love.event.poll(): function",
        documentation: r"Returns an iterator for messages in the event queue.",
    },
    LoveApiItem {
        path: &["love", "event"],
        label: "pump",
        kind: LuaItemKind::Function,
        detail: r"function love.event.pump()",
        documentation: r"Pump events into the event queue.

This is a low-level function, and is usually not called by the user, but by love.run.

Note that this does need to be called for any OS to think you're still running,

and if you want to handle OS-generated events at all (think callbacks).",
    },
    LoveApiItem {
        path: &["love", "event"],
        label: "push",
        kind: LuaItemKind::Function,
        detail: r"function love.event.push(n: Event, a?: any, b?: any, c?: any, d?: any, e?: any, f?: any, ...)",
        documentation: r"Adds an event to the event queue.

From 0.10.0 onwards, you may pass an arbitrary amount of arguments with this function, though the default callbacks don't ever use more than six.",
    },
    LoveApiItem {
        path: &["love", "event"],
        label: "quit",
        kind: LuaItemKind::Function,
        detail: r"function love.event.quit(exitstatus?: number)",
        documentation: r"Adds the quit event to the queue.

The quit event is a signal for the event handler to close LÖVE. It's possible to abort the exit process with the love.quit callback.",
    },
    LoveApiItem {
        path: &["love", "event"],
        label: "wait",
        kind: LuaItemKind::Function,
        detail: r"function love.event.wait(): Event, any, any, any, any, any, any, any",
        documentation: r"Like love.event.poll(), but blocks until there is an event in the queue.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "append",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.append(name: string, data: string, size?: number): boolean, string",
        documentation: r"Append data to an existing file.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "areSymlinksEnabled",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.areSymlinksEnabled(): boolean",
        documentation: r"Gets whether love.filesystem follows symbolic links.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "createDirectory",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.createDirectory(name: string): boolean",
        documentation: r"Recursively creates a directory.

When called with 'a/b' it creates both 'a' and 'a/b', if they don't exist already.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getAppdataDirectory",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getAppdataDirectory(): string",
        documentation: r"Returns the application data directory (could be the same as getUserDirectory)",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getCRequirePath",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getCRequirePath(): string",
        documentation: r"Gets the filesystem paths that will be searched for c libraries when require is called.

The paths string returned by this function is a sequence of path templates separated by semicolons. The argument passed to ''require'' will be inserted in place of any question mark ('?') character in each template (after the dot characters in the argument passed to ''require'' are replaced by directory separators.) Additionally, any occurrence of a double question mark ('??') will be replaced by the name passed to require and the default library extension for the platform.

The paths are relative to the game's source and save directories, as well as any paths mounted with love.filesystem.mount.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getDirectoryItems",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getDirectoryItems(dir: string): table",
        documentation: r"Returns a table with the names of files and subdirectories in the specified path. The table is not sorted in any way; the order is undefined.

If the path passed to the function exists in the game and the save directory, it will list the files and directories from both places.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getIdentity",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getIdentity(): string",
        documentation: r"Gets the write directory name for your game. 

Note that this only returns the name of the folder to store your files in, not the full path.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getInfo",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getInfo(path: string, filtertype?: FileType): table",
        documentation: r"Gets information about the specified file or directory.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getRealDirectory",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getRealDirectory(filepath: string): string",
        documentation: r"Gets the platform-specific absolute path of the directory containing a filepath.

This can be used to determine whether a file is inside the save directory or the game's source .love.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getRequirePath",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getRequirePath(): string",
        documentation: r"Gets the filesystem paths that will be searched when require is called.

The paths string returned by this function is a sequence of path templates separated by semicolons. The argument passed to ''require'' will be inserted in place of any question mark ('?') character in each template (after the dot characters in the argument passed to ''require'' are replaced by directory separators.)

The paths are relative to the game's source and save directories, as well as any paths mounted with love.filesystem.mount.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getSaveDirectory",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getSaveDirectory(): string",
        documentation: r"Gets the full path to the designated save directory.

This can be useful if you want to use the standard io library (or something else) to

read or write in the save directory.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getSource",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getSource(): string",
        documentation: r"Returns the full path to the the .love file or directory. If the game is fused to the LÖVE executable, then the executable is returned.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getSourceBaseDirectory",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getSourceBaseDirectory(): string",
        documentation: r"Returns the full path to the directory containing the .love file. If the game is fused to the LÖVE executable, then the directory containing the executable is returned.

If love.filesystem.isFused is true, the path returned by this function can be passed to love.filesystem.mount, which will make the directory containing the main game (e.g. C:\Program Files\coolgame\) readable by love.filesystem.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getUserDirectory",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getUserDirectory(): string",
        documentation: r"Returns the path of the user's directory",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "getWorkingDirectory",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.getWorkingDirectory(): string",
        documentation: r"Gets the current working directory.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "init",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.init(appname: string)",
        documentation: r"Initializes love.filesystem, will be called internally, so should not be used explicitly.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "isFused",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.isFused(): boolean",
        documentation: r"Gets whether the game is in fused mode or not.

If a game is in fused mode, its save directory will be directly in the Appdata directory instead of Appdata/LOVE/. The game will also be able to load C Lua dynamic libraries which are located in the save directory.

A game is in fused mode if the source .love has been fused to the executable (see Game Distribution), or if '--fused' has been given as a command-line argument when starting the game.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "lines",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.lines(name: string): function",
        documentation: r"Iterate over the lines in a file.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "load",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.load(name: string): function, string",
        documentation: r"Loads a Lua file (but does not run it).",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "mount",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.mount(archive: string, mountpoint: string, appendToPath?: boolean): boolean",
        documentation: r"Mounts a zip file or folder in the game's save directory for reading.

It is also possible to mount love.filesystem.getSourceBaseDirectory if the game is in fused mode.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "newFile",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.newFile(filename: string): File",
        documentation: r"Creates a new File object. 

It needs to be opened before it can be accessed.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "newFileData",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.newFileData(contents: string, name: string): FileData",
        documentation: r"Creates a new FileData object from a file on disk, or from a string in memory.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "read",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.read(name: string, size?: number): string, number, nil, string",
        documentation: r"Read the contents of a file.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "remove",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.remove(name: string): boolean",
        documentation: r"Removes a file or empty directory.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "setCRequirePath",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.setCRequirePath(paths: string)",
        documentation: r"Sets the filesystem paths that will be searched for c libraries when require is called.

The paths string returned by this function is a sequence of path templates separated by semicolons. The argument passed to ''require'' will be inserted in place of any question mark ('?') character in each template (after the dot characters in the argument passed to ''require'' are replaced by directory separators.) Additionally, any occurrence of a double question mark ('??') will be replaced by the name passed to require and the default library extension for the platform.

The paths are relative to the game's source and save directories, as well as any paths mounted with love.filesystem.mount.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "setIdentity",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.setIdentity(name: string)",
        documentation: r"Sets the write directory for your game. 

Note that you can only set the name of the folder to store your files in, not the location.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "setRequirePath",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.setRequirePath(paths: string)",
        documentation: r"Sets the filesystem paths that will be searched when require is called.

The paths string given to this function is a sequence of path templates separated by semicolons. The argument passed to ''require'' will be inserted in place of any question mark ('?') character in each template (after the dot characters in the argument passed to ''require'' are replaced by directory separators.)

The paths are relative to the game's source and save directories, as well as any paths mounted with love.filesystem.mount.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "setSource",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.setSource(path: string)",
        documentation: r"Sets the source of the game, where the code is present. This function can only be called once, and is normally automatically done by LÖVE.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "setSymlinksEnabled",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.setSymlinksEnabled(enable: boolean)",
        documentation: r"Sets whether love.filesystem follows symbolic links. It is enabled by default in version 0.10.0 and newer, and disabled by default in 0.9.2.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "unmount",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.unmount(archive: string): boolean",
        documentation: r"Unmounts a zip file or folder previously mounted for reading with love.filesystem.mount.",
    },
    LoveApiItem {
        path: &["love", "filesystem"],
        label: "write",
        kind: LuaItemKind::Function,
        detail: r"function love.filesystem.write(name: string, data: string, size?: number): boolean, string",
        documentation: r"Write data to a file in the save directory. If the file existed already, it will be completely replaced by the new contents.",
    },
    LoveApiItem {
        path: &["love", "font"],
        label: "newBMFontRasterizer",
        kind: LuaItemKind::Function,
        detail: r"function love.font.newBMFontRasterizer(imageData: ImageData, glyphs: string, dpiscale?: number): Rasterizer",
        documentation: r"Creates a new BMFont Rasterizer.",
    },
    LoveApiItem {
        path: &["love", "font"],
        label: "newGlyphData",
        kind: LuaItemKind::Function,
        detail: r"function love.font.newGlyphData(rasterizer: Rasterizer, glyph: number)",
        documentation: r"Creates a new GlyphData.",
    },
    LoveApiItem {
        path: &["love", "font"],
        label: "newImageRasterizer",
        kind: LuaItemKind::Function,
        detail: r"function love.font.newImageRasterizer(imageData: ImageData, glyphs: string, extraSpacing?: number, dpiscale?: number): Rasterizer",
        documentation: r"Creates a new Image Rasterizer.",
    },
    LoveApiItem {
        path: &["love", "font"],
        label: "newRasterizer",
        kind: LuaItemKind::Function,
        detail: r"function love.font.newRasterizer(filename: string): Rasterizer",
        documentation: r"Creates a new Rasterizer.",
    },
    LoveApiItem {
        path: &["love", "font"],
        label: "newTrueTypeRasterizer",
        kind: LuaItemKind::Function,
        detail: r"function love.font.newTrueTypeRasterizer(size?: number, hinting?: HintingMode, dpiscale?: number): Rasterizer",
        documentation: r"Creates a new TrueType Rasterizer.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "applyTransform",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.applyTransform(transform: Transform)",
        documentation: r"Applies the given Transform object to the current coordinate transformation.

This effectively multiplies the existing coordinate transformation's matrix with the Transform object's internal matrix to produce the new coordinate transformation.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "arc",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.arc(drawmode: DrawMode, x: number, y: number, radius: number, angle1: number, angle2: number, segments?: number)",
        documentation: r"Draws a filled or unfilled arc at position (x, y). The arc is drawn from angle1 to angle2 in radians. The segments parameter determines how many segments are used to draw the arc. The more segments, the smoother the edge.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "captureScreenshot",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.captureScreenshot(filename: string)",
        documentation: r"Creates a screenshot once the current frame is done (after love.draw has finished).

Since this function enqueues a screenshot capture rather than executing it immediately, it can be called from an input callback or love.update and it will still capture all of what's drawn to the screen in that frame.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "circle",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.circle(mode: DrawMode, x: number, y: number, radius: number)",
        documentation: r"Draws a circle.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "clear",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.clear()",
        documentation: r"Clears the screen or active Canvas to the specified color.

This function is called automatically before love.draw in the default love.run function. See the example in love.run for a typical use of this function.

Note that the scissor area bounds the cleared region.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.

In versions prior to background color instead.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "discard",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.discard(discardcolor?: boolean, discardstencil?: boolean)",
        documentation: r"Discards (trashes) the contents of the screen or active Canvas. This is a performance optimization function with niche use cases.

If the active Canvas has just been changed and the 'replace' BlendMode is about to be used to draw something which covers the entire screen, calling love.graphics.discard rather than calling love.graphics.clear or doing nothing may improve performance on mobile devices.

On some desktop systems this function may do nothing.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "draw",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.draw(drawable: Drawable, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)",
        documentation: r"Draws a Drawable object (an Image, Canvas, SpriteBatch, ParticleSystem, Mesh, Text object, or Video) on the screen with optional rotation, scaling and shearing.

Objects are drawn relative to their local coordinate system. The origin is by default located at the top left corner of Image and Canvas. All scaling, shearing, and rotation arguments transform the object relative to that point. Also, the position of the origin can be specified on the screen coordinate system.

It's possible to rotate an object about its center by offsetting the origin to the center. Angles must be given in radians for rotation. One can also use a negative scaling factor to flip about its centerline. 

Note that the offsets are applied before rotation, scaling, or shearing; scaling and shearing are applied before rotation.

The right and bottom edges of the object are shifted at an angle defined by the shearing factors.

When using the default shader anything drawn with this function will be tinted according to the currently selected color.  Set it to pure white to preserve the object's original colors.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "drawInstanced",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.drawInstanced(mesh: Mesh, instancecount: number, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)",
        documentation: r"Draws many instances of a Mesh with a single draw call, using hardware geometry instancing.

Each instance can have unique properties (positions, colors, etc.) but will not by default unless a custom per-instance vertex attributes or the love_InstanceID GLSL 3 vertex shader variable is used, otherwise they will all render at the same position on top of each other.

Instancing is not supported by some older GPUs that are only capable of using OpenGL ES 2 or OpenGL 2. Use love.graphics.getSupported to check.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "drawLayer",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.drawLayer(texture: Texture, layerindex: number, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)",
        documentation: r"Draws a layer of an Array Texture.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "ellipse",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.ellipse(mode: DrawMode, x: number, y: number, radiusx: number, radiusy: number)",
        documentation: r"Draws an ellipse.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "flushBatch",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.flushBatch()",
        documentation: r"Immediately renders any pending automatically batched draws.

LÖVE will call this function internally as needed when most state is changed, so it is not necessary to manually call it.

The current batch will be automatically flushed by love.graphics state changes (except for the transform stack and the current color), as well as Shader:send and methods on Textures which change their state. Using a different Image in consecutive love.graphics.draw calls will also flush the current batch.

SpriteBatches, ParticleSystems, Meshes, and Text objects do their own batching and do not affect automatic batching of other draws, aside from flushing the current batch when they're drawn.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getBackgroundColor",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getBackgroundColor(): number, number, number, number",
        documentation: r"Gets the current background color.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getBlendMode",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getBlendMode(): BlendMode, BlendAlphaMode",
        documentation: r"Gets the blending mode.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getCanvas",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getCanvas(): Canvas",
        documentation: r"Gets the current target Canvas.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getCanvasFormats",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getCanvasFormats(): table",
        documentation: r"Gets the available Canvas formats, and whether each is supported.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getColor",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getColor(): number, number, number, number",
        documentation: r"Gets the current color.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getColorMask",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getColorMask(): boolean, boolean, boolean, boolean",
        documentation: r"Gets the active color components used when drawing. Normally all 4 components are active unless love.graphics.setColorMask has been used.

The color mask determines whether individual components of the colors of drawn objects will affect the color of the screen. They affect love.graphics.clear and Canvas:clear as well.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getDPIScale",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getDPIScale(): number",
        documentation: r"Gets the DPI scale factor of the window.

The DPI scale factor represents relative pixel density. The pixel density inside the window might be greater (or smaller) than the 'size' of the window. For example on a retina screen in Mac OS X with the highdpi window flag enabled, the window may take up the same physical size as an 800x600 window, but the area inside the window uses 1600x1200 pixels. love.graphics.getDPIScale() would return 2 in that case.

The love.window.fromPixels and love.window.toPixels functions can also be used to convert between units.

The highdpi window flag must be enabled to use the full pixel density of a Retina screen on Mac OS X and iOS. The flag currently does nothing on Windows and Linux, and on Android it is effectively always enabled.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getDefaultFilter",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getDefaultFilter(): FilterMode, FilterMode, number",
        documentation: r"Returns the default scaling filters used with Images, Canvases, and Fonts.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getDepthMode",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getDepthMode(): CompareMode, boolean",
        documentation: r"Gets the current depth test mode and whether writing to the depth buffer is enabled.

This is low-level functionality designed for use with custom vertex shaders and Meshes with custom vertex attributes. No higher level APIs are provided to set the depth of 2D graphics such as shapes, lines, and Images.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getDimensions",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getDimensions(): number, number",
        documentation: r"Gets the width and height in pixels of the window.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getFont",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getFont(): Font",
        documentation: r"Gets the current Font object.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getFrontFaceWinding",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getFrontFaceWinding(): VertexWinding",
        documentation: r"Gets whether triangles with clockwise- or counterclockwise-ordered vertices are considered front-facing.

This is designed for use in combination with Mesh face culling. Other love.graphics shapes, lines, and sprites are not guaranteed to have a specific winding order to their internal vertices.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getHeight",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getHeight(): number",
        documentation: r"Gets the height in pixels of the window.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getImageFormats",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getImageFormats(): table",
        documentation: r"Gets the raw and compressed pixel formats usable for Images, and whether each is supported.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getLineJoin",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getLineJoin(): LineJoin",
        documentation: r"Gets the line join style.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getLineStyle",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getLineStyle(): LineStyle",
        documentation: r"Gets the line style.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getLineWidth",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getLineWidth(): number",
        documentation: r"Gets the current line width.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getMeshCullMode",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getMeshCullMode(): CullMode",
        documentation: r"Gets whether back-facing triangles in a Mesh are culled.

Mesh face culling is designed for use with low level custom hardware-accelerated 3D rendering via custom vertex attributes on Meshes, custom vertex shaders, and depth testing with a depth buffer.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getPixelDimensions",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getPixelDimensions(): number, number",
        documentation: r"Gets the width and height in pixels of the window.

love.graphics.getDimensions gets the dimensions of the window in units scaled by the screen's DPI scale factor, rather than pixels. Use getDimensions for calculations related to drawing to the screen and using the graphics coordinate system (calculating the center of the screen, for example), and getPixelDimensions only when dealing specifically with underlying pixels (pixel-related calculations in a pixel Shader, for example).",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getPixelHeight",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getPixelHeight(): number",
        documentation: r"Gets the height in pixels of the window.

The graphics coordinate system and DPI scale factor, rather than raw pixels. Use getHeight for calculations related to drawing to the screen and using the coordinate system (calculating the center of the screen, for example), and getPixelHeight only when dealing specifically with underlying pixels (pixel-related calculations in a pixel Shader, for example).",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getPixelWidth",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getPixelWidth(): number",
        documentation: r"Gets the width in pixels of the window.

The graphics coordinate system and DPI scale factor, rather than raw pixels. Use getWidth for calculations related to drawing to the screen and using the coordinate system (calculating the center of the screen, for example), and getPixelWidth only when dealing specifically with underlying pixels (pixel-related calculations in a pixel Shader, for example).",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getPointSize",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getPointSize(): number",
        documentation: r"Gets the point size.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getRendererInfo",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getRendererInfo(): string, string, string, string",
        documentation: r"Gets information about the system's video card and drivers.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getScissor",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getScissor(): number, number, number, number",
        documentation: r"Gets the current scissor box.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getShader",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getShader(): Shader",
        documentation: r"Gets the current Shader. Returns nil if none is set.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getStackDepth",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getStackDepth(): number",
        documentation: r"Gets the current depth of the transform / state stack (the number of pushes without corresponding pops).",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getStats",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getStats(): table",
        documentation: r"Gets performance-related rendering statistics.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getStencilTest",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getStencilTest(): CompareMode, number",
        documentation: r"Gets the current stencil test configuration.

When stencil testing is enabled, the geometry of everything that is drawn afterward will be clipped / stencilled out based on a comparison between the arguments of this function and the stencil value of each pixel that the geometry touches. The stencil values of pixels are affected via love.graphics.stencil.

Each Canvas has its own per-pixel stencil values.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getSupported",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getSupported(): table",
        documentation: r"Gets the optional graphics features and whether they're supported on the system.

Some older or low-end systems don't always support all graphics features.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getSystemLimits",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getSystemLimits(): table",
        documentation: r"Gets the system-dependent maximum values for love.graphics features.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getTextureTypes",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getTextureTypes(): table",
        documentation: r"Gets the available texture types, and whether each is supported.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "getWidth",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.getWidth(): number",
        documentation: r"Gets the width in pixels of the window.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "intersectScissor",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.intersectScissor(x: number, y: number, width: number, height: number)",
        documentation: r"Sets the scissor to the rectangle created by the intersection of the specified rectangle with the existing scissor.  If no scissor is active yet, it behaves like love.graphics.setScissor.

The scissor limits the drawing area to a specified rectangle. This affects all graphics calls, including love.graphics.clear.

The dimensions of the scissor is unaffected by graphical transformations (translate, scale, ...).",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "inverseTransformPoint",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.inverseTransformPoint(screenX: number, screenY: number): number, number",
        documentation: r"Converts the given 2D position from screen-space into global coordinates.

This effectively applies the reverse of the current graphics transformations to the given position. A similar Transform:inverseTransformPoint method exists for Transform objects.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "isActive",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.isActive(): boolean",
        documentation: r"Gets whether the graphics module is able to be used. If it is not active, love.graphics function and method calls will not work correctly and may cause the program to crash.
The graphics module is inactive if a window is not open, or if the app is in the background on iOS. Typically the app's execution will be automatically paused by the system, in the latter case.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "isGammaCorrect",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.isGammaCorrect(): boolean",
        documentation: r"Gets whether gamma-correct rendering is supported and enabled. It can be enabled by setting t.gammacorrect = true in love.conf.

Not all devices support gamma-correct rendering, in which case it will be automatically disabled and this function will return false. It is supported on desktop systems which have graphics cards that are capable of using OpenGL 3 / DirectX 10, and iOS devices that can use OpenGL ES 3.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "isWireframe",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.isWireframe(): boolean",
        documentation: r"Gets whether wireframe mode is used when drawing.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "line",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.line(x1: number, y1: number, x2: number, y2: number, ...)",
        documentation: r"Draws lines between points.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newArrayImage",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newArrayImage(slices: table, settings?: table): Image",
        documentation: r"Creates a new array Image.

An array image / array texture is a single object which contains multiple 'layers' or 'slices' of 2D sub-images. It can be thought of similarly to a texture atlas or sprite sheet, but it doesn't suffer from the same tile / quad bleeding artifacts that texture atlases do – although every sub-image must have the same dimensions.

A specific layer of an array image can be drawn with love.graphics.drawLayer / SpriteBatch:addLayer, or with the Quad variant of love.graphics.draw and Quad:setLayer, or via a custom Shader.

To use an array image in a Shader, it must be declared as a ArrayImage or sampler2DArray type (instead of Image or sampler2D). The Texel(ArrayImage image, vec3 texturecoord) shader function must be used to get pixel colors from a slice of the array image. The vec3 argument contains the texture coordinate in the first two components, and the 0-based slice index in the third component.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newCanvas",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newCanvas(): Canvas",
        documentation: r"Creates a new Canvas object for offscreen rendering.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newCubeImage",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newCubeImage(filename: string, settings?: table): Image",
        documentation: r"Creates a new cubemap Image.

Cubemap images have 6 faces (sides) which represent a cube. They can't be rendered directly, they can only be used in Shader code (and sent to the shader via Shader:send).

To use a cubemap image in a Shader, it must be declared as a CubeImage or samplerCube type (instead of Image or sampler2D). The Texel(CubeImage image, vec3 direction) shader function must be used to get pixel colors from the cubemap. The vec3 argument is a normalized direction from the center of the cube, rather than explicit texture coordinates.

Each face in a cubemap image must have square dimensions.

For variants of this function which accept a single image containing multiple cubemap faces, they must be laid out in one of the following forms in the image:

   +y

+z +x -z

   -y

   -x

or:

   +y

-x +z +x -z

   -y

or:

+x

-x

+y

-y

+z

-z

or:

+x -x +y -y +z -z",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newFont",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newFont(filename: string): Font",
        documentation: r"Creates a new Font from a TrueType Font or BMFont file. Created fonts are not cached, in that calling this function with the same arguments will always create a new Font object.

All variants which accept a filename can also accept a Data object instead.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newImage",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newImage(filename: string, settings?: table): Image",
        documentation: r"Creates a new Image from a filepath, FileData, an ImageData, or a CompressedImageData, and optionally generates or specifies mipmaps for the image.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newImageFont",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newImageFont(filename: string, glyphs: string, extraspacing?: number): Font",
        documentation: r"Creates a new specifically formatted image.

In versions prior to 0.9.0, LÖVE expects ISO 8859-1 encoding for the glyphs string.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newMesh",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newMesh(vertices: table, mode?: MeshDrawMode, usage?: SpriteBatchUsage): Mesh",
        documentation: r"Creates a new Mesh.

Use Mesh:setTexture if the Mesh should be textured with an Image or Canvas when it's drawn.

In versions prior to 11.0, color and byte component values were within the range of 0 to 255 instead of 0 to 1.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newParticleSystem",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newParticleSystem(image: Image, buffer?: number): ParticleSystem",
        documentation: r"Creates a new ParticleSystem.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newQuad",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newQuad(x: number, y: number, width: number, height: number, sw: number, sh: number): Quad",
        documentation: r"Creates a new Quad.

The purpose of a Quad is to use a fraction of an image to draw objects, as opposed to drawing entire image. It is most useful for sprite sheets and atlases: in a sprite atlas, multiple sprites reside in same image, quad is used to draw a specific sprite from that image; in animated sprites with all frames residing in the same image, quad is used to draw specific frame from the animation.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newShader",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newShader(code: string): Shader",
        documentation: r"Creates a new Shader object for hardware-accelerated vertex and pixel effects. A Shader contains either vertex shader code, pixel shader code, or both.

Shaders are small programs which are run on the graphics card when drawing. Vertex shaders are run once for each vertex (for example, an image has 4 vertices - one at each corner. A Mesh might have many more.) Pixel shaders are run once for each pixel on the screen which the drawn object touches. Pixel shader code is executed after all the object's vertices have been processed by the vertex shader.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newSpriteBatch",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newSpriteBatch(image: Image, maxsprites?: number): SpriteBatch",
        documentation: r"Creates a new SpriteBatch object.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newText",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newText(font: Font, textstring?: string): Text",
        documentation: r"Creates a new drawable Text object.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newVideo",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newVideo(filename: string): Video",
        documentation: r"Creates a new drawable Video. Currently only Ogg Theora video files are supported.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "newVolumeImage",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.newVolumeImage(layers: table, settings?: table): Image",
        documentation: r"Creates a new volume (3D) Image.

Volume images are 3D textures with width, height, and depth. They can't be rendered directly, they can only be used in Shader code (and sent to the shader via Shader:send).

To use a volume image in a Shader, it must be declared as a VolumeImage or sampler3D type (instead of Image or sampler2D). The Texel(VolumeImage image, vec3 texcoords) shader function must be used to get pixel colors from the volume image. The vec3 argument is a normalized texture coordinate with the z component representing the depth to sample at (ranging from 1).

Volume images are typically used as lookup tables in shaders for color grading, for example, because sampling using a texture coordinate that is partway in between two pixels can interpolate across all 3 dimensions in the volume image, resulting in a smooth gradient even when a small-sized volume image is used as the lookup table.

Array images are a much better choice than volume images for storing multiple different sprites in a single array image for directly drawing them.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "origin",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.origin()",
        documentation: r"Resets the current coordinate transformation.

This function is always used to reverse any previous calls to love.graphics.rotate, love.graphics.scale, love.graphics.shear or love.graphics.translate. It returns the current transformation state to its defaults.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "points",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.points(x: number, y: number, ...)",
        documentation: r"Draws one or more points.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "polygon",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.polygon(mode: DrawMode, ...)",
        documentation: r"Draw a polygon.

Following the mode argument, this function can accept multiple numeric arguments or a single table of numeric arguments. In either case the arguments are interpreted as alternating x and y coordinates of the polygon's vertices.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "pop",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.pop()",
        documentation: r"Pops the current coordinate transformation from the transformation stack.

This function is always used to reverse a previous push operation. It returns the current transformation state to what it was before the last preceding push.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "present",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.present()",
        documentation: r"Displays the results of drawing operations on the screen.

This function is used when writing your own love.run function. It presents all the results of your drawing operations on the screen. See the example in love.run for a typical use of this function.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "print",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.print(text: string, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)",
        documentation: r"Draws text on screen. If no Font is set, one will be created and set (once) if needed.

As of LOVE 0.7.1, when using translation and scaling functions while drawing text, this function assumes the scale occurs first.  If you don't script with this in mind, the text won't be in the right position, or possibly even on screen.

love.graphics.print and love.graphics.printf both support UTF-8 encoding. You'll also need a proper Font for special characters.

In versions prior to 11.0, color and byte component values were within the range of 0 to 255 instead of 0 to 1.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "printf",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.printf(text: string, x: number, y: number, limit: number, align?: AlignMode, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)",
        documentation: r"Draws formatted text, with word wrap and alignment.

See additional notes in love.graphics.print.

The word wrap limit is applied before any scaling, rotation, and other coordinate transformations. Therefore the amount of text per line stays constant given the same wrap limit, even if the scale arguments change.

In version 0.9.2 and earlier, wrapping was implemented by breaking up words by spaces and putting them back together to make sure things fit nicely within the limit provided. However, due to the way this is done, extra spaces between words would end up missing when printed on the screen, and some lines could overflow past the provided wrap limit. In version 0.10.0 and newer this is no longer the case.

In versions prior to 11.0, color and byte component values were within the range of 0 to 255 instead of 0 to 1.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "push",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.push()",
        documentation: r"Copies and pushes the current coordinate transformation to the transformation stack.

This function is always used to prepare for a corresponding pop operation later. It stores the current coordinate transformation state into the transformation stack and keeps it active. Later changes to the transformation can be undone by using the pop operation, which returns the coordinate transform to the state it was in before calling push.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "rectangle",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.rectangle(mode: DrawMode, x: number, y: number, width: number, height: number)",
        documentation: r"Draws a rectangle.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "replaceTransform",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.replaceTransform(transform: Transform)",
        documentation: r"Replaces the current coordinate transformation with the given Transform object.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "reset",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.reset()",
        documentation: r"Resets the current graphics settings.

Calling reset makes the current drawing color white, the current background color black, disables any active color component masks, disables wireframe mode and resets the current graphics transformation to the origin. It also sets both the point and line drawing modes to smooth and their sizes to 1.0.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "rotate",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.rotate(angle: number)",
        documentation: r"Rotates the coordinate system in two dimensions.

Calling this function affects all future drawing operations by rotating the coordinate system around the origin by the given amount of radians. This change lasts until love.draw() exits.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "scale",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.scale(sx: number, sy?: number)",
        documentation: r"Scales the coordinate system in two dimensions.

By default the coordinate system in LÖVE corresponds to the display pixels in horizontal and vertical directions one-to-one, and the x-axis increases towards the right while the y-axis increases downwards. Scaling the coordinate system changes this relation.

After scaling by sx and sy, all coordinates are treated as if they were multiplied by sx and sy. Every result of a drawing operation is also correspondingly scaled, so scaling by (2, 2) for example would mean making everything twice as large in both x- and y-directions. Scaling by a negative value flips the coordinate system in the corresponding direction, which also means everything will be drawn flipped or upside down, or both. Scaling by zero is not a useful operation.

Scale and translate are not commutative operations, therefore, calling them in different orders will change the outcome.

Scaling lasts until love.draw() exits.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setBackgroundColor",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setBackgroundColor(red: number, green: number, blue: number, alpha?: number)",
        documentation: r"Sets the background color.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setBlendMode",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setBlendMode(mode: BlendMode)",
        documentation: r"Sets the blending mode.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setCanvas",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setCanvas(canvas: Canvas, mipmap?: number)",
        documentation: r"Captures drawing operations to a Canvas.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setColor",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setColor(red: number, green: number, blue: number, alpha?: number)",
        documentation: r"Sets the color used for drawing.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setColorMask",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setColorMask(red: boolean, green: boolean, blue: boolean, alpha: boolean)",
        documentation: r"Sets the color mask. Enables or disables specific color components when rendering and clearing the screen. For example, if '''red''' is set to '''false''', no further changes will be made to the red component of any pixels.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setDefaultFilter",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setDefaultFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)",
        documentation: r"Sets the default scaling filters used with Images, Canvases, and Fonts.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setDepthMode",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setDepthMode(comparemode: CompareMode, write: boolean)",
        documentation: r"Configures depth testing and writing to the depth buffer.

This is low-level functionality designed for use with custom vertex shaders and Meshes with custom vertex attributes. No higher level APIs are provided to set the depth of 2D graphics such as shapes, lines, and Images.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setFont",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setFont(font: Font)",
        documentation: r"Set an already-loaded Font as the current font or create and load a new one from the file and size.

It's recommended that Font objects are created with love.graphics.newFont in the loading stage and then passed to this function in the drawing stage.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setFrontFaceWinding",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setFrontFaceWinding(winding: VertexWinding)",
        documentation: r"Sets whether triangles with clockwise- or counterclockwise-ordered vertices are considered front-facing.

This is designed for use in combination with Mesh face culling. Other love.graphics shapes, lines, and sprites are not guaranteed to have a specific winding order to their internal vertices.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setLineJoin",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setLineJoin(join: LineJoin)",
        documentation: r"Sets the line join style. See LineJoin for the possible options.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setLineStyle",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setLineStyle(style: LineStyle)",
        documentation: r"Sets the line style.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setLineWidth",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setLineWidth(width: number)",
        documentation: r"Sets the line width.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setMeshCullMode",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setMeshCullMode(mode: CullMode)",
        documentation: r"Sets whether back-facing triangles in a Mesh are culled.

This is designed for use with low level custom hardware-accelerated 3D rendering via custom vertex attributes on Meshes, custom vertex shaders, and depth testing with a depth buffer.

By default, both front- and back-facing triangles in Meshes are rendered.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setNewFont",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setNewFont(size?: number): Font",
        documentation: r"Creates and sets a new Font.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setPointSize",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setPointSize(size: number)",
        documentation: r"Sets the point size.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setScissor",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setScissor(x: number, y: number, width: number, height: number)",
        documentation: r"Sets or disables scissor.

The scissor limits the drawing area to a specified rectangle. This affects all graphics calls, including love.graphics.clear. 

The dimensions of the scissor is unaffected by graphical transformations (translate, scale, ...).",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setShader",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setShader(shader: Shader)",
        documentation: r"Sets or resets a Shader as the current pixel effect or vertex shaders. All drawing operations until the next ''love.graphics.setShader'' will be drawn using the Shader object specified.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setStencilTest",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setStencilTest(comparemode: CompareMode, comparevalue: number)",
        documentation: r"Configures or disables stencil testing.

When stencil testing is enabled, the geometry of everything that is drawn afterward will be clipped / stencilled out based on a comparison between the arguments of this function and the stencil value of each pixel that the geometry touches. The stencil values of pixels are affected via love.graphics.stencil.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "setWireframe",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.setWireframe(enable: boolean)",
        documentation: r"Sets whether wireframe lines will be used when drawing.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "shear",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.shear(kx: number, ky: number)",
        documentation: r"Shears the coordinate system.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "stencil",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.stencil(stencilfunction: function, action?: StencilAction, value?: number, keepvalues?: boolean)",
        documentation: r"Draws geometry as a stencil.

The geometry drawn by the supplied function sets invisible stencil values of pixels, instead of setting pixel colors. The stencil buffer (which contains those stencil values) can act like a mask / stencil - love.graphics.setStencilTest can be used afterward to determine how further rendering is affected by the stencil values in each pixel.

Stencil values are integers within the range of 255.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "transformPoint",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.transformPoint(globalX: number, globalY: number): number, number",
        documentation: r"Converts the given 2D position from global coordinates into screen-space.

This effectively applies the current graphics transformations to the given position. A similar Transform:transformPoint method exists for Transform objects.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "translate",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.translate(dx: number, dy: number)",
        documentation: r"Translates the coordinate system in two dimensions.

When this function is called with two numbers, dx, and dy, all the following drawing operations take effect as if their x and y coordinates were x+dx and y+dy. 

Scale and translate are not commutative operations, therefore, calling them in different orders will change the outcome.

This change lasts until love.draw() exits or else a love.graphics.pop reverts to a previous love.graphics.push.

Translating using whole numbers will prevent tearing/blurring of images and fonts draw after translating.",
    },
    LoveApiItem {
        path: &["love", "graphics"],
        label: "validateShader",
        kind: LuaItemKind::Function,
        detail: r"function love.graphics.validateShader(gles: boolean, code: string): boolean, string",
        documentation: r"Validates shader code. Check if specified shader code does not contain any errors.",
    },
    LoveApiItem {
        path: &["love", "image"],
        label: "isCompressed",
        kind: LuaItemKind::Function,
        detail: r"function love.image.isCompressed(filename: string): boolean",
        documentation: r"Determines whether a file can be loaded as CompressedImageData.",
    },
    LoveApiItem {
        path: &["love", "image"],
        label: "newCompressedData",
        kind: LuaItemKind::Function,
        detail: r"function love.image.newCompressedData(filename: string): CompressedImageData",
        documentation: r"Create a new CompressedImageData object from a compressed image file. LÖVE supports several compressed texture formats, enumerated in the CompressedImageFormat page.",
    },
    LoveApiItem {
        path: &["love", "image"],
        label: "newImageData",
        kind: LuaItemKind::Function,
        detail: r"function love.image.newImageData(width: number, height: number): ImageData",
        documentation: r"Creates a new ImageData object.",
    },
    LoveApiItem {
        path: &["love", "joystick"],
        label: "getGamepadMappingString",
        kind: LuaItemKind::Function,
        detail: r"function love.joystick.getGamepadMappingString(guid: string): string",
        documentation: r"Gets the full gamepad mapping string of the Joysticks which have the given GUID, or nil if the GUID isn't recognized as a gamepad.

The mapping string contains binding information used to map the Joystick's buttons an axes to the standard gamepad layout, and can be used later with love.joystick.loadGamepadMappings.",
    },
    LoveApiItem {
        path: &["love", "joystick"],
        label: "getJoystickCount",
        kind: LuaItemKind::Function,
        detail: r"function love.joystick.getJoystickCount(): number",
        documentation: r"Gets the number of connected joysticks.",
    },
    LoveApiItem {
        path: &["love", "joystick"],
        label: "getJoysticks",
        kind: LuaItemKind::Function,
        detail: r"function love.joystick.getJoysticks(): table",
        documentation: r"Gets a list of connected Joysticks.",
    },
    LoveApiItem {
        path: &["love", "joystick"],
        label: "loadGamepadMappings",
        kind: LuaItemKind::Function,
        detail: r"function love.joystick.loadGamepadMappings(filename: string)",
        documentation: r"Loads a gamepad mappings string or file created with love.joystick.saveGamepadMappings.

It also recognizes any SDL gamecontroller mapping string, such as those created with Steam's Big Picture controller configure interface, or this nice database. If a new mapping is loaded for an already known controller GUID, the later version will overwrite the one currently loaded.",
    },
    LoveApiItem {
        path: &["love", "joystick"],
        label: "saveGamepadMappings",
        kind: LuaItemKind::Function,
        detail: r"function love.joystick.saveGamepadMappings(filename: string): string",
        documentation: r"Saves the virtual gamepad mappings of all recognized as gamepads and have either been recently used or their gamepad bindings have been modified.

The mappings are stored as a string for use with love.joystick.loadGamepadMappings.",
    },
    LoveApiItem {
        path: &["love", "joystick"],
        label: "setGamepadMapping",
        kind: LuaItemKind::Function,
        detail: r"function love.joystick.setGamepadMapping(guid: string, button: GamepadButton, inputtype: JoystickInputType, inputindex: number, hatdir?: JoystickHat): boolean",
        documentation: r"Binds a virtual gamepad input to a button, axis or hat for all Joysticks of a certain type. For example, if this function is used with a GUID returned by a Dualshock 3 controller in OS X, the binding will affect Joystick:getGamepadAxis and Joystick:isGamepadDown for ''all'' Dualshock 3 controllers used with the game when run in OS X.

LÖVE includes built-in gamepad bindings for many common controllers. This function lets you change the bindings or add new ones for types of Joysticks which aren't recognized as gamepads by default.

The virtual gamepad buttons and axes are designed around the Xbox 360 controller layout.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "getKeyFromScancode",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.getKeyFromScancode(scancode: Scancode): KeyConstant",
        documentation: r"Gets the key corresponding to the given hardware scancode.

Unlike key constants, Scancodes are keyboard layout-independent. For example the scancode 'w' will be generated if the key in the same place as the 'w' key on an American keyboard is pressed, no matter what the key is labelled or what the user's operating system settings are.

Scancodes are useful for creating default controls that have the same physical locations on on all systems.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "getScancodeFromKey",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.getScancodeFromKey(key: KeyConstant): Scancode",
        documentation: r"Gets the hardware scancode corresponding to the given key.

Unlike key constants, Scancodes are keyboard layout-independent. For example the scancode 'w' will be generated if the key in the same place as the 'w' key on an American keyboard is pressed, no matter what the key is labelled or what the user's operating system settings are.

Scancodes are useful for creating default controls that have the same physical locations on on all systems.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "hasKeyRepeat",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.hasKeyRepeat(): boolean",
        documentation: r"Gets whether key repeat is enabled.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "hasScreenKeyboard",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.hasScreenKeyboard(): boolean",
        documentation: r"Gets whether screen keyboard is supported.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "hasTextInput",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.hasTextInput(): boolean",
        documentation: r"Gets whether text input events are enabled.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "isDown",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.isDown(key: KeyConstant): boolean",
        documentation: r"Checks whether a certain key is down. Not to be confused with love.keypressed or love.keyreleased.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "isScancodeDown",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.isScancodeDown(scancode: Scancode, ...): boolean",
        documentation: r"Checks whether the specified Scancodes are pressed. Not to be confused with love.keypressed or love.keyreleased.

Unlike regular KeyConstants, Scancodes are keyboard layout-independent. The scancode 'w' is used if the key in the same place as the 'w' key on an American keyboard is pressed, no matter what the key is labelled or what the user's operating system settings are.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "setKeyRepeat",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.setKeyRepeat(enable: boolean)",
        documentation: r"Enables or disables key repeat for love.keypressed. It is disabled by default.",
    },
    LoveApiItem {
        path: &["love", "keyboard"],
        label: "setTextInput",
        kind: LuaItemKind::Function,
        detail: r"function love.keyboard.setTextInput(enable: boolean)",
        documentation: r"Enables or disables text input events. It is enabled by default on Windows, Mac, and Linux, and disabled by default on iOS and Android.

On touch devices, this shows the system's native on-screen keyboard when it's enabled.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "colorFromBytes",
        kind: LuaItemKind::Function,
        detail: r"function love.math.colorFromBytes(rb: number, gb: number, bb: number, ab?: number): number, number, number, number",
        documentation: r"Converts a color from 0..255 to 0..1 range.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "colorToBytes",
        kind: LuaItemKind::Function,
        detail: r"function love.math.colorToBytes(r: number, g: number, b: number, a?: number): number, number, number, number",
        documentation: r"Converts a color from 0..1 to 0..255 range.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "gammaToLinear",
        kind: LuaItemKind::Function,
        detail: r"function love.math.gammaToLinear(r: number, g: number, b: number): number, number, number",
        documentation: r"Converts a color from gamma-space (sRGB) to linear-space (RGB). This is useful when doing gamma-correct rendering and you need to do math in linear RGB in the few cases where LÖVE doesn't handle conversions automatically.

Read more about gamma-correct rendering here, here, and here.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "getRandomSeed",
        kind: LuaItemKind::Function,
        detail: r"function love.math.getRandomSeed(): number, number",
        documentation: r"Gets the seed of the random number generator.

The seed is split into two numbers due to Lua's use of doubles for all number values - doubles can't accurately represent integer  values above 2^53, but the seed can be an integer value up to 2^64.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "getRandomState",
        kind: LuaItemKind::Function,
        detail: r"function love.math.getRandomState(): string",
        documentation: r"Gets the current state of the random number generator. This returns an opaque implementation-dependent string which is only useful for later use with love.math.setRandomState or RandomGenerator:setState.

This is different from love.math.getRandomSeed in that getRandomState gets the random number generator's current state, whereas getRandomSeed gets the previously set seed number.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "isConvex",
        kind: LuaItemKind::Function,
        detail: r"function love.math.isConvex(vertices: table): boolean",
        documentation: r"Checks whether a polygon is convex.

PolygonShapes in love.physics, some forms of Meshes, and polygons drawn with love.graphics.polygon must be simple convex polygons.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "linearToGamma",
        kind: LuaItemKind::Function,
        detail: r"function love.math.linearToGamma(lr: number, lg: number, lb: number): number, number, number",
        documentation: r"Converts a color from linear-space (RGB) to gamma-space (sRGB). This is useful when storing linear RGB color values in an image, because the linear RGB color space has less precision than sRGB for dark colors, which can result in noticeable color banding when drawing.

In general, colors chosen based on what they look like on-screen are already in gamma-space and should not be double-converted. Colors calculated using math are often in the linear RGB space.

Read more about gamma-correct rendering here, here, and here.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "newBezierCurve",
        kind: LuaItemKind::Function,
        detail: r"function love.math.newBezierCurve(vertices: table): BezierCurve",
        documentation: r"Creates a new BezierCurve object.

The number of vertices in the control polygon determines the degree of the curve, e.g. three vertices define a quadratic (degree 2) Bézier curve, four vertices define a cubic (degree 3) Bézier curve, etc.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "newRandomGenerator",
        kind: LuaItemKind::Function,
        detail: r"function love.math.newRandomGenerator(): RandomGenerator",
        documentation: r"Creates a new RandomGenerator object which is completely independent of other RandomGenerator objects and random functions.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "newTransform",
        kind: LuaItemKind::Function,
        detail: r"function love.math.newTransform(): Transform",
        documentation: r"Creates a new Transform object.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "noise",
        kind: LuaItemKind::Function,
        detail: r"function love.math.noise(x: number): number",
        documentation: r"Generates a Simplex or Perlin noise value in 1-4 dimensions. The return value will always be the same, given the same arguments.

Simplex noise is closely related to Perlin noise. It is widely used for procedural content generation.

There are many webpages which discuss Perlin and Simplex noise in detail.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "random",
        kind: LuaItemKind::Function,
        detail: r"function love.math.random(): number",
        documentation: r"Generates a pseudo-random number in a platform independent manner. The default love.run seeds this function at startup, so you generally don't need to seed it yourself.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "randomNormal",
        kind: LuaItemKind::Function,
        detail: r"function love.math.randomNormal(stddev?: number, mean?: number): number",
        documentation: r"Get a normally distributed pseudo random number.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "setRandomSeed",
        kind: LuaItemKind::Function,
        detail: r"function love.math.setRandomSeed(seed: number)",
        documentation: r"Sets the seed of the random number generator using the specified integer number. This is called internally at startup, so you generally don't need to call it yourself.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "setRandomState",
        kind: LuaItemKind::Function,
        detail: r"function love.math.setRandomState(state: string)",
        documentation: r"Sets the current state of the random number generator. The value used as an argument for this function is an opaque implementation-dependent string and should only originate from a previous call to love.math.getRandomState.

This is different from love.math.setRandomSeed in that setRandomState directly sets the random number generator's current implementation-dependent state, whereas setRandomSeed gives it a new seed value.",
    },
    LoveApiItem {
        path: &["love", "math"],
        label: "triangulate",
        kind: LuaItemKind::Function,
        detail: r"function love.math.triangulate(polygon: table): table",
        documentation: r"Decomposes a simple convex or concave polygon into triangles.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "getCursor",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.getCursor(): Cursor",
        documentation: r"Gets the current Cursor.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "getPosition",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.getPosition(): number, number",
        documentation: r"Returns the current position of the mouse.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "getRelativeMode",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.getRelativeMode(): boolean",
        documentation: r"Gets whether relative mode is enabled for the mouse.

If relative mode is enabled, the cursor is hidden and doesn't move when the mouse does, but relative mouse motion events are still generated via love.mousemoved. This lets the mouse move in any direction indefinitely without the cursor getting stuck at the edges of the screen.

The reported position of the mouse is not updated while relative mode is enabled, even when relative mouse motion events are generated.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "getSystemCursor",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.getSystemCursor(ctype: CursorType): Cursor",
        documentation: r"Gets a Cursor object representing a system-native hardware cursor.

Hardware cursors are framerate-independent and work the same way as normal operating system cursors. Unlike drawing an image at the mouse's current coordinates, hardware cursors never have visible lag between when the mouse is moved and when the cursor position updates, even at low framerates.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "getX",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.getX(): number",
        documentation: r"Returns the current x-position of the mouse.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "getY",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.getY(): number",
        documentation: r"Returns the current y-position of the mouse.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "isCursorSupported",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.isCursorSupported(): boolean",
        documentation: r"Gets whether cursor functionality is supported.

If it isn't supported, calling love.mouse.newCursor and love.mouse.getSystemCursor will cause an error. Mobile devices do not support cursors.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "isDown",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.isDown(button: number, ...): boolean",
        documentation: r"Checks whether a certain mouse button is down.

This function does not detect mouse wheel scrolling; you must use the love.wheelmoved (or love.mousepressed in version 0.9.2 and older) callback for that.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "isGrabbed",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.isGrabbed(): boolean",
        documentation: r"Checks if the mouse is grabbed.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "isVisible",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.isVisible(): boolean",
        documentation: r"Checks if the cursor is visible.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "newCursor",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.newCursor(imageData: ImageData, hotx?: number, hoty?: number): Cursor",
        documentation: r"Creates a new hardware Cursor object from an image file or ImageData.

Hardware cursors are framerate-independent and work the same way as normal operating system cursors. Unlike drawing an image at the mouse's current coordinates, hardware cursors never have visible lag between when the mouse is moved and when the cursor position updates, even at low framerates.

The hot spot is the point the operating system uses to determine what was clicked and at what position the mouse cursor is. For example, the normal arrow pointer normally has its hot spot at the top left of the image, but a crosshair cursor might have it in the middle.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "setCursor",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.setCursor(cursor: Cursor)",
        documentation: r"Sets the current mouse cursor.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "setGrabbed",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.setGrabbed(grab: boolean)",
        documentation: r"Grabs the mouse and confines it to the window.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "setPosition",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.setPosition(x: number, y: number)",
        documentation: r"Sets the current position of the mouse. Non-integer values are floored.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "setRelativeMode",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.setRelativeMode(enable: boolean)",
        documentation: r"Sets whether relative mode is enabled for the mouse.

When relative mode is enabled, the cursor is hidden and doesn't move when the mouse does, but relative mouse motion events are still generated via love.mousemoved. This lets the mouse move in any direction indefinitely without the cursor getting stuck at the edges of the screen.

The reported position of the mouse may not be updated while relative mode is enabled, even when relative mouse motion events are generated.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "setVisible",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.setVisible(visible: boolean)",
        documentation: r"Sets the current visibility of the cursor.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "setX",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.setX(x: number)",
        documentation: r"Sets the current X position of the mouse.

Non-integer values are floored.",
    },
    LoveApiItem {
        path: &["love", "mouse"],
        label: "setY",
        kind: LuaItemKind::Function,
        detail: r"function love.mouse.setY(y: number)",
        documentation: r"Sets the current Y position of the mouse.

Non-integer values are floored.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "getDistance",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.getDistance(fixture1: Fixture, fixture2: Fixture): number, number, number, number, number",
        documentation: r"Returns the two closest points between two fixtures and their distance.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "getMeter",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.getMeter(): number",
        documentation: r"Returns the meter scale factor.

All coordinates in the physics module are divided by this number, creating a convenient way to draw the objects directly to the screen without the need for graphics transformations.

It is recommended to create shapes no larger than 10 times the scale. This is important because Box2D is tuned to work well with shape sizes from 0.1 to 10 meters.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newBody",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newBody(world: World, x?: number, y?: number, type?: BodyType): Body",
        documentation: r"Creates a new body.

There are three types of bodies. 

* Static bodies do not move, have a infinite mass, and can be used for level boundaries. 

* Dynamic bodies are the main actors in the simulation, they collide with everything. 

* Kinematic bodies do not react to forces and only collide with dynamic bodies.

The mass of the body gets calculated when a Fixture is attached or removed, but can be changed at any time with Body:setMass or Body:resetMassData.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newChainShape",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newChainShape(loop: boolean, x1: number, y1: number, x2: number, y2: number, ...): ChainShape",
        documentation: r"Creates a new ChainShape.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newCircleShape",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newCircleShape(radius: number): CircleShape",
        documentation: r"Creates a new CircleShape.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newDistanceJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newDistanceJoint(body1: Body, body2: Body, x1: number, y1: number, x2: number, y2: number, collideConnected?: boolean): DistanceJoint",
        documentation: r"Creates a DistanceJoint between two bodies.

This joint constrains the distance between two points on two bodies to be constant. These two points are specified in world coordinates and the two bodies are assumed to be in place when this joint is created. The first anchor point is connected to the first body and the second to the second body, and the points define the length of the distance joint.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newEdgeShape",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newEdgeShape(x1: number, y1: number, x2: number, y2: number): EdgeShape",
        documentation: r"Creates a new EdgeShape.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newFixture",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newFixture(body: Body, shape: Shape, density?: number): Fixture",
        documentation: r"Creates and attaches a Fixture to a body.

Note that the Shape object is copied rather than kept as a reference when the Fixture is created. To get the Shape object that the Fixture owns, use Fixture:getShape.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newFrictionJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newFrictionJoint(body1: Body, body2: Body, x: number, y: number, collideConnected?: boolean): FrictionJoint",
        documentation: r"Create a friction joint between two bodies. A FrictionJoint applies friction to a body.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newGearJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newGearJoint(joint1: Joint, joint2: Joint, ratio?: number, collideConnected?: boolean): GearJoint",
        documentation: r"Create a GearJoint connecting two Joints.

The gear joint connects two joints that must be either  prismatic or  revolute joints. Using this joint requires that the joints it uses connect their respective bodies to the ground and have the ground as the first body. When destroying the bodies and joints you must make sure you destroy the gear joint before the other joints.

The gear joint has a ratio the determines how the angular or distance values of the connected joints relate to each other. The formula coordinate1 + ratio * coordinate2 always has a constant value that is set when the gear joint is created.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newMotorJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newMotorJoint(body1: Body, body2: Body, correctionFactor?: number): MotorJoint",
        documentation: r"Creates a joint between two bodies which controls the relative motion between them.

Position and rotation offsets can be specified once the MotorJoint has been created, as well as the maximum motor force and torque that will be be applied to reach the target offsets.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newMouseJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newMouseJoint(body: Body, x: number, y: number): MouseJoint",
        documentation: r"Create a joint between a body and the mouse.

This joint actually connects the body to a fixed point in the world. To make it follow the mouse, the fixed point must be updated every timestep (example below).

The advantage of using a MouseJoint instead of just changing a body position directly is that collisions and reactions to other joints are handled by the physics engine.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newPolygonShape",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newPolygonShape(x1: number, y1: number, x2: number, y2: number, x3: number, y3: number, ...): PolygonShape",
        documentation: r"Creates a new PolygonShape.

This shape can have 8 vertices at most, and must form a convex shape.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newPrismaticJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newPrismaticJoint(body1: Body, body2: Body, x: number, y: number, ax: number, ay: number, collideConnected?: boolean): PrismaticJoint",
        documentation: r"Creates a PrismaticJoint between two bodies.

A prismatic joint constrains two bodies to move relatively to each other on a specified axis. It does not allow for relative rotation. Its definition and operation are similar to a  revolute joint, but with translation and force substituted for angle and torque.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newPulleyJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newPulleyJoint(body1: Body, body2: Body, gx1: number, gy1: number, gx2: number, gy2: number, x1: number, y1: number, x2: number, y2: number, ratio?: number, collideConnected?: boolean): PulleyJoint",
        documentation: r"Creates a PulleyJoint to join two bodies to each other and the ground.

The pulley joint simulates a pulley with an optional block and tackle. If the ratio parameter has a value different from one, then the simulated rope extends faster on one side than the other. In a pulley joint the total length of the simulated rope is the constant length1 + ratio * length2, which is set when the pulley joint is created.

Pulley joints can behave unpredictably if one side is fully extended. It is recommended that the method  setMaxLengths  be used to constrain the maximum lengths each side can attain.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newRectangleShape",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newRectangleShape(width: number, height: number): PolygonShape",
        documentation: r"Shorthand for creating rectangular PolygonShapes. 

By default, the local origin is located at the '''center''' of the rectangle as opposed to the top left for graphics.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newRevoluteJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newRevoluteJoint(body1: Body, body2: Body, x: number, y: number, collideConnected?: boolean): RevoluteJoint",
        documentation: r"Creates a pivot joint between two bodies.

This joint connects two bodies to a point around which they can pivot.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newRopeJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newRopeJoint(body1: Body, body2: Body, x1: number, y1: number, x2: number, y2: number, maxLength: number, collideConnected?: boolean): RopeJoint",
        documentation: r"Creates a joint between two bodies. Its only function is enforcing a max distance between these bodies.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newWeldJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newWeldJoint(body1: Body, body2: Body, x: number, y: number, collideConnected?: boolean): WeldJoint",
        documentation: r"Creates a constraint joint between two bodies. A WeldJoint essentially glues two bodies together. The constraint is a bit soft, however, due to Box2D's iterative solver.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newWheelJoint",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newWheelJoint(body1: Body, body2: Body, x: number, y: number, ax: number, ay: number, collideConnected?: boolean): WheelJoint",
        documentation: r"Creates a wheel joint.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "newWorld",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.newWorld(xg?: number, yg?: number, sleep?: boolean): World",
        documentation: r"Creates a new World.",
    },
    LoveApiItem {
        path: &["love", "physics"],
        label: "setMeter",
        kind: LuaItemKind::Function,
        detail: r"function love.physics.setMeter(scale: number)",
        documentation: r"Sets the pixels to meter scale factor.

All coordinates in the physics module are divided by this number and converted to meters, and it creates a convenient way to draw the objects directly to the screen without the need for graphics transformations.

It is recommended to create shapes no larger than 10 times the scale. This is important because Box2D is tuned to work well with shape sizes from 0.1 to 10 meters. The default meter scale is 30.",
    },
    LoveApiItem {
        path: &["love", "sound"],
        label: "newDecoder",
        kind: LuaItemKind::Function,
        detail: r"function love.sound.newDecoder(file: File, buffer?: number): Decoder",
        documentation: r"Attempts to find a decoder for the encoded sound data in the specified file.",
    },
    LoveApiItem {
        path: &["love", "sound"],
        label: "newSoundData",
        kind: LuaItemKind::Function,
        detail: r"function love.sound.newSoundData(filename: string): SoundData",
        documentation: r"Creates new SoundData from a filepath, File, or Decoder. It's also possible to create SoundData with a custom sample rate, channel and bit depth.

The sound data will be decoded to the memory in a raw format. It is recommended to create only short sounds like effects, as a 3 minute song uses 30 MB of memory this way.",
    },
    LoveApiItem {
        path: &["love", "system"],
        label: "getClipboardText",
        kind: LuaItemKind::Function,
        detail: r"function love.system.getClipboardText(): string",
        documentation: r"Gets text from the clipboard.",
    },
    LoveApiItem {
        path: &["love", "system"],
        label: "getOS",
        kind: LuaItemKind::Function,
        detail: r"function love.system.getOS(): string",
        documentation: r"Gets the current operating system. In general, LÖVE abstracts away the need to know the current operating system, but there are a few cases where it can be useful (especially in combination with os.execute.)",
    },
    LoveApiItem {
        path: &["love", "system"],
        label: "getPowerInfo",
        kind: LuaItemKind::Function,
        detail: r"function love.system.getPowerInfo(): PowerState, number, number",
        documentation: r"Gets information about the system's power supply.",
    },
    LoveApiItem {
        path: &["love", "system"],
        label: "getProcessorCount",
        kind: LuaItemKind::Function,
        detail: r"function love.system.getProcessorCount(): number",
        documentation: r"Gets the amount of logical processor in the system.",
    },
    LoveApiItem {
        path: &["love", "system"],
        label: "hasBackgroundMusic",
        kind: LuaItemKind::Function,
        detail: r"function love.system.hasBackgroundMusic(): boolean",
        documentation: r"Gets whether another application on the system is playing music in the background.

Currently this is implemented on iOS and Android, and will always return false on other operating systems. The t.audio.mixwithsystem flag in love.conf can be used to configure whether background audio / music from other apps should play while LÖVE is open.",
    },
    LoveApiItem {
        path: &["love", "system"],
        label: "openURL",
        kind: LuaItemKind::Function,
        detail: r"function love.system.openURL(url: string): boolean",
        documentation: r"Opens a URL with the user's web or file browser.",
    },
    LoveApiItem {
        path: &["love", "system"],
        label: "setClipboardText",
        kind: LuaItemKind::Function,
        detail: r"function love.system.setClipboardText(text: string)",
        documentation: r"Puts text in the clipboard.",
    },
    LoveApiItem {
        path: &["love", "system"],
        label: "vibrate",
        kind: LuaItemKind::Function,
        detail: r"function love.system.vibrate(seconds?: number)",
        documentation: r"Causes the device to vibrate, if possible. Currently this will only work on Android and iOS devices that have a built-in vibration motor.",
    },
    LoveApiItem {
        path: &["love", "thread"],
        label: "getChannel",
        kind: LuaItemKind::Function,
        detail: r"function love.thread.getChannel(name: string): Channel",
        documentation: r"Creates or retrieves a named thread channel.",
    },
    LoveApiItem {
        path: &["love", "thread"],
        label: "newChannel",
        kind: LuaItemKind::Function,
        detail: r"function love.thread.newChannel(): Channel",
        documentation: r"Create a new unnamed thread channel.

One use for them is to pass new unnamed channels to other threads via Channel:push on a named channel.",
    },
    LoveApiItem {
        path: &["love", "thread"],
        label: "newThread",
        kind: LuaItemKind::Function,
        detail: r"function love.thread.newThread(filename: string): Thread",
        documentation: r"Creates a new Thread from a filename, string or FileData object containing Lua code.",
    },
    LoveApiItem {
        path: &["love", "timer"],
        label: "getAverageDelta",
        kind: LuaItemKind::Function,
        detail: r"function love.timer.getAverageDelta(): number",
        documentation: r"Returns the average delta time (seconds per frame) over the last second.",
    },
    LoveApiItem {
        path: &["love", "timer"],
        label: "getDelta",
        kind: LuaItemKind::Function,
        detail: r"function love.timer.getDelta(): number",
        documentation: r"Returns the time between the last two frames.",
    },
    LoveApiItem {
        path: &["love", "timer"],
        label: "getFPS",
        kind: LuaItemKind::Function,
        detail: r"function love.timer.getFPS(): number",
        documentation: r"Returns the current frames per second.",
    },
    LoveApiItem {
        path: &["love", "timer"],
        label: "getTime",
        kind: LuaItemKind::Function,
        detail: r"function love.timer.getTime(): number",
        documentation: r"Returns the value of a timer with an unspecified starting time.

This function should only be used to calculate differences between points in time, as the starting time of the timer is unknown.",
    },
    LoveApiItem {
        path: &["love", "timer"],
        label: "sleep",
        kind: LuaItemKind::Function,
        detail: r"function love.timer.sleep(s: number)",
        documentation: r"Pauses the current thread for the specified amount of time.",
    },
    LoveApiItem {
        path: &["love", "timer"],
        label: "step",
        kind: LuaItemKind::Function,
        detail: r"function love.timer.step(): number",
        documentation: r"Measures the time between two frames.

Calling this changes the return value of love.timer.getDelta.",
    },
    LoveApiItem {
        path: &["love", "touch"],
        label: "getPosition",
        kind: LuaItemKind::Function,
        detail: r"function love.touch.getPosition(id: light userdata): number, number",
        documentation: r"Gets the current position of the specified touch-press, in pixels.",
    },
    LoveApiItem {
        path: &["love", "touch"],
        label: "getPressure",
        kind: LuaItemKind::Function,
        detail: r"function love.touch.getPressure(id: light userdata): number",
        documentation: r"Gets the current pressure of the specified touch-press.",
    },
    LoveApiItem {
        path: &["love", "touch"],
        label: "getTouches",
        kind: LuaItemKind::Function,
        detail: r"function love.touch.getTouches(): table",
        documentation: r"Gets a list of all active touch-presses.",
    },
    LoveApiItem {
        path: &["love", "video"],
        label: "newVideoStream",
        kind: LuaItemKind::Function,
        detail: r"function love.video.newVideoStream(filename: string): VideoStream",
        documentation: r"Creates a new VideoStream. Currently only Ogg Theora video files are supported. VideoStreams can't draw videos, see love.graphics.newVideo for that.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "close",
        kind: LuaItemKind::Function,
        detail: r"function love.window.close()",
        documentation: r"Closes the window. It can be reopened with love.window.setMode.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "fromPixels",
        kind: LuaItemKind::Function,
        detail: r"function love.window.fromPixels(pixelvalue: number): number",
        documentation: r"Converts a number from pixels to density-independent units.

The pixel density inside the window might be greater (or smaller) than the 'size' of the window. For example on a retina screen in Mac OS X with the highdpi window flag enabled, the window may take up the same physical size as an 800x600 window, but the area inside the window uses 1600x1200 pixels. love.window.fromPixels(1600) would return 800 in that case.

This function converts coordinates from pixels to the size users are expecting them to display at onscreen. love.window.toPixels does the opposite. The highdpi window flag must be enabled to use the full pixel density of a Retina screen on Mac OS X and iOS. The flag currently does nothing on Windows and Linux, and on Android it is effectively always enabled.

Most LÖVE functions return values and expect arguments in terms of pixels rather than density-independent units.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getDPIScale",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getDPIScale(): number",
        documentation: r"Gets the DPI scale factor associated with the window.

The pixel density inside the window might be greater (or smaller) than the 'size' of the window. For example on a retina screen in Mac OS X with the highdpi window flag enabled, the window may take up the same physical size as an 800x600 window, but the area inside the window uses 1600x1200 pixels. love.window.getDPIScale() would return 2.0 in that case.

The love.window.fromPixels and love.window.toPixels functions can also be used to convert between units.

The highdpi window flag must be enabled to use the full pixel density of a Retina screen on Mac OS X and iOS. The flag currently does nothing on Windows and Linux, and on Android it is effectively always enabled.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getDesktopDimensions",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getDesktopDimensions(displayindex?: number): number, number",
        documentation: r"Gets the width and height of the desktop.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getDisplayCount",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getDisplayCount(): number",
        documentation: r"Gets the number of connected monitors.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getDisplayName",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getDisplayName(displayindex?: number): string",
        documentation: r"Gets the name of a display.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getDisplayOrientation",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getDisplayOrientation(displayindex?: number): DisplayOrientation",
        documentation: r"Gets current device display orientation.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getFullscreen",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getFullscreen(): boolean, FullscreenType",
        documentation: r"Gets whether the window is fullscreen.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getFullscreenModes",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getFullscreenModes(displayindex?: number): table",
        documentation: r"Gets a list of supported fullscreen modes.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getIcon",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getIcon(): ImageData",
        documentation: r"Gets the window icon.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getMode",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getMode(): number, number, table",
        documentation: r"Gets the display mode and properties of the window.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getPosition",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getPosition(): number, number, number",
        documentation: r"Gets the position of the window on the screen.

The window position is in the coordinate space of the display it is currently in.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getSafeArea",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getSafeArea(): number, number, number, number",
        documentation: r"Gets area inside the window which is known to be unobstructed by a system title bar, the iPhone X notch, etc. Useful for making sure UI elements can be seen by the user.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getTitle",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getTitle(): string",
        documentation: r"Gets the window title.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "getVSync",
        kind: LuaItemKind::Function,
        detail: r"function love.window.getVSync(): number",
        documentation: r"Gets current vertical synchronization (vsync).",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "hasFocus",
        kind: LuaItemKind::Function,
        detail: r"function love.window.hasFocus(): boolean",
        documentation: r"Checks if the game window has keyboard focus.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "hasMouseFocus",
        kind: LuaItemKind::Function,
        detail: r"function love.window.hasMouseFocus(): boolean",
        documentation: r"Checks if the game window has mouse focus.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "isDisplaySleepEnabled",
        kind: LuaItemKind::Function,
        detail: r"function love.window.isDisplaySleepEnabled(): boolean",
        documentation: r"Gets whether the display is allowed to sleep while the program is running.

Display sleep is disabled by default. Some types of input (e.g. joystick button presses) might not prevent the display from sleeping, if display sleep is allowed.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "isMaximized",
        kind: LuaItemKind::Function,
        detail: r"function love.window.isMaximized(): boolean",
        documentation: r"Gets whether the Window is currently maximized.

The window can be maximized if it is not fullscreen and is resizable, and either the user has pressed the window's Maximize button or love.window.maximize has been called.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "isMinimized",
        kind: LuaItemKind::Function,
        detail: r"function love.window.isMinimized(): boolean",
        documentation: r"Gets whether the Window is currently minimized.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "isOpen",
        kind: LuaItemKind::Function,
        detail: r"function love.window.isOpen(): boolean",
        documentation: r"Checks if the window is open.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "isVisible",
        kind: LuaItemKind::Function,
        detail: r"function love.window.isVisible(): boolean",
        documentation: r"Checks if the game window is visible.

The window is considered visible if it's not minimized and the program isn't hidden.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "maximize",
        kind: LuaItemKind::Function,
        detail: r"function love.window.maximize()",
        documentation: r"Makes the window as large as possible.

This function has no effect if the window isn't resizable, since it essentially programmatically presses the window's 'maximize' button.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "minimize",
        kind: LuaItemKind::Function,
        detail: r"function love.window.minimize()",
        documentation: r"Minimizes the window to the system's task bar / dock.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "requestAttention",
        kind: LuaItemKind::Function,
        detail: r"function love.window.requestAttention(continuous?: boolean)",
        documentation: r"Causes the window to request the attention of the user if it is not in the foreground.

In Windows the taskbar icon will flash, and in OS X the dock icon will bounce.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "restore",
        kind: LuaItemKind::Function,
        detail: r"function love.window.restore()",
        documentation: r"Restores the size and position of the window if it was minimized or maximized.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "setDisplaySleepEnabled",
        kind: LuaItemKind::Function,
        detail: r"function love.window.setDisplaySleepEnabled(enable: boolean)",
        documentation: r"Sets whether the display is allowed to sleep while the program is running.

Display sleep is disabled by default. Some types of input (e.g. joystick button presses) might not prevent the display from sleeping, if display sleep is allowed.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "setFullscreen",
        kind: LuaItemKind::Function,
        detail: r"function love.window.setFullscreen(fullscreen: boolean): boolean",
        documentation: r"Enters or exits fullscreen. The display to use when entering fullscreen is chosen based on which display the window is currently in, if multiple monitors are connected.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "setIcon",
        kind: LuaItemKind::Function,
        detail: r"function love.window.setIcon(imagedata: ImageData): boolean",
        documentation: r"Sets the window icon until the game is quit. Not all operating systems support very large icon images.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "setMode",
        kind: LuaItemKind::Function,
        detail: r"function love.window.setMode(width: number, height: number, flags: table): boolean",
        documentation: r"Sets the display mode and properties of the window.

If width or height is 0, setMode will use the width and height of the desktop. 

Changing the display mode may have side effects: for example, canvases will be cleared and values sent to shaders with canvases beforehand or re-draw to them afterward if you need to.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "setPosition",
        kind: LuaItemKind::Function,
        detail: r"function love.window.setPosition(x: number, y: number, displayindex?: number)",
        documentation: r"Sets the position of the window on the screen.

The window position is in the coordinate space of the specified display.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "setTitle",
        kind: LuaItemKind::Function,
        detail: r"function love.window.setTitle(title: string)",
        documentation: r"Sets the window title.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "setVSync",
        kind: LuaItemKind::Function,
        detail: r"function love.window.setVSync(vsync: number)",
        documentation: r"Sets vertical synchronization mode.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "showMessageBox",
        kind: LuaItemKind::Function,
        detail: r"function love.window.showMessageBox(title: string, message: string, type?: MessageBoxType, attachtowindow?: boolean): boolean",
        documentation: r"Displays a message box dialog above the love window. The message box contains a title, optional text, and buttons.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "toPixels",
        kind: LuaItemKind::Function,
        detail: r"function love.window.toPixels(value: number): number",
        documentation: r"Converts a number from density-independent units to pixels.

The pixel density inside the window might be greater (or smaller) than the 'size' of the window. For example on a retina screen in Mac OS X with the highdpi window flag enabled, the window may take up the same physical size as an 800x600 window, but the area inside the window uses 1600x1200 pixels. love.window.toPixels(800) would return 1600 in that case.

This is used to convert coordinates from the size users are expecting them to display at onscreen to pixels. love.window.fromPixels does the opposite. The highdpi window flag must be enabled to use the full pixel density of a Retina screen on Mac OS X and iOS. The flag currently does nothing on Windows and Linux, and on Android it is effectively always enabled.

Most LÖVE functions return values and expect arguments in terms of pixels rather than density-independent units.",
    },
    LoveApiItem {
        path: &["love", "window"],
        label: "updateMode",
        kind: LuaItemKind::Function,
        detail: r"function love.window.updateMode(width: number, height: number, settings: table): boolean",
        documentation: r"Sets the display mode and properties of the window, without modifying unspecified properties.

If width or height is 0, updateMode will use the width and height of the desktop. 

Changing the display mode may have side effects: for example, canvases will be cleared. Make sure to save the contents of canvases beforehand or re-draw to them afterward if you need to.",
    },
];
