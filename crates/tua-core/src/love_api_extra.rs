// Generated from love2d-community/love-api 11.5 modules/enums and modules/types.
// Source revision: 4b2d89510eabd320791c4a8157e6e6656c34ec09
// Source data: https://github.com/love2d-community/love-api
// Regenerate with: make generate-love-api LOVE_API_ROOT=/path/to/love-api

use crate::lua::{LuaItem, LuaItemKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LoveApiTypedItem {
    type_name: &'static str,
    item: LuaItem,
}

pub(crate) fn type_names() -> &'static [&'static str] {
    TYPE_NAMES
}

pub(crate) fn type_supertypes() -> &'static [(&'static str, &'static str)] {
    TYPE_SUPERTYPES
}

pub(crate) fn type_field_items(type_name: &str, prefix: &str) -> Vec<LuaItem> {
    TYPE_ITEMS
        .iter()
        .filter(|entry| entry.type_name == type_name)
        .map(|entry| entry.item)
        .filter(|item| item.label.starts_with(prefix))
        .collect()
}

pub(crate) fn enum_value_items(type_name: &str, prefix: &str) -> Vec<LuaItem> {
    ENUM_ITEMS
        .iter()
        .filter(|entry| entry.type_name == type_name)
        .map(|entry| entry.item)
        .filter(|item| item.label.starts_with(prefix))
        .collect()
}

pub(crate) fn all_items() -> impl Iterator<Item = LuaItem> {
    TYPE_ITEMS
        .iter()
        .chain(ENUM_ITEMS.iter())
        .map(|entry| entry.item)
}

const TYPE_NAMES: &[&str] = &[
    r"BezierCurve",
    r"Body",
    r"ByteData",
    r"Canvas",
    r"ChainShape",
    r"Channel",
    r"CircleShape",
    r"CompressedData",
    r"CompressedImageData",
    r"Contact",
    r"Cursor",
    r"Data",
    r"Decoder",
    r"DistanceJoint",
    r"Drawable",
    r"DroppedFile",
    r"EdgeShape",
    r"File",
    r"FileData",
    r"Fixture",
    r"Font",
    r"FrictionJoint",
    r"GearJoint",
    r"GlyphData",
    r"Image",
    r"ImageData",
    r"Joint",
    r"Joystick",
    r"Mesh",
    r"MotorJoint",
    r"MouseJoint",
    r"Object",
    r"ParticleSystem",
    r"PolygonShape",
    r"PrismaticJoint",
    r"PulleyJoint",
    r"Quad",
    r"RandomGenerator",
    r"Rasterizer",
    r"RecordingDevice",
    r"RevoluteJoint",
    r"RopeJoint",
    r"Shader",
    r"Shape",
    r"SoundData",
    r"Source",
    r"SpriteBatch",
    r"Text",
    r"Texture",
    r"Thread",
    r"Transform",
    r"Video",
    r"VideoStream",
    r"WeldJoint",
    r"WheelJoint",
    r"World",
];

const TYPE_SUPERTYPES: &[(&str, &str)] = &[
    (r"BezierCurve", r"Object"),
    (r"Body", r"Object"),
    (r"ByteData", r"Object"),
    (r"ByteData", r"Data"),
    (r"Canvas", r"Texture"),
    (r"Canvas", r"Drawable"),
    (r"Canvas", r"Object"),
    (r"ChainShape", r"Shape"),
    (r"ChainShape", r"Object"),
    (r"Channel", r"Object"),
    (r"CircleShape", r"Shape"),
    (r"CircleShape", r"Object"),
    (r"CompressedData", r"Data"),
    (r"CompressedData", r"Object"),
    (r"CompressedImageData", r"Data"),
    (r"CompressedImageData", r"Object"),
    (r"Contact", r"Object"),
    (r"Cursor", r"Object"),
    (r"Data", r"Object"),
    (r"Decoder", r"Object"),
    (r"DistanceJoint", r"Joint"),
    (r"DistanceJoint", r"Object"),
    (r"Drawable", r"Object"),
    (r"DroppedFile", r"File"),
    (r"DroppedFile", r"Object"),
    (r"EdgeShape", r"Shape"),
    (r"EdgeShape", r"Object"),
    (r"File", r"Object"),
    (r"FileData", r"Data"),
    (r"FileData", r"Object"),
    (r"Fixture", r"Object"),
    (r"Font", r"Object"),
    (r"FrictionJoint", r"Joint"),
    (r"FrictionJoint", r"Object"),
    (r"GearJoint", r"Joint"),
    (r"GearJoint", r"Object"),
    (r"GlyphData", r"Data"),
    (r"GlyphData", r"Object"),
    (r"Image", r"Texture"),
    (r"Image", r"Drawable"),
    (r"Image", r"Object"),
    (r"ImageData", r"Data"),
    (r"ImageData", r"Object"),
    (r"Joint", r"Object"),
    (r"Joystick", r"Object"),
    (r"Mesh", r"Drawable"),
    (r"Mesh", r"Object"),
    (r"MotorJoint", r"Joint"),
    (r"MotorJoint", r"Object"),
    (r"MouseJoint", r"Joint"),
    (r"MouseJoint", r"Object"),
    (r"ParticleSystem", r"Drawable"),
    (r"ParticleSystem", r"Object"),
    (r"PolygonShape", r"Shape"),
    (r"PolygonShape", r"Object"),
    (r"PrismaticJoint", r"Joint"),
    (r"PrismaticJoint", r"Object"),
    (r"PulleyJoint", r"Joint"),
    (r"PulleyJoint", r"Object"),
    (r"Quad", r"Object"),
    (r"RandomGenerator", r"Object"),
    (r"Rasterizer", r"Object"),
    (r"RecordingDevice", r"Object"),
    (r"RevoluteJoint", r"Joint"),
    (r"RevoluteJoint", r"Object"),
    (r"RopeJoint", r"Joint"),
    (r"RopeJoint", r"Object"),
    (r"Shader", r"Object"),
    (r"Shape", r"Object"),
    (r"SoundData", r"Data"),
    (r"SoundData", r"Object"),
    (r"Source", r"Object"),
    (r"SpriteBatch", r"Drawable"),
    (r"SpriteBatch", r"Object"),
    (r"Text", r"Drawable"),
    (r"Text", r"Object"),
    (r"Texture", r"Drawable"),
    (r"Texture", r"Object"),
    (r"Thread", r"Object"),
    (r"Transform", r"Object"),
    (r"Video", r"Drawable"),
    (r"Video", r"Object"),
    (r"VideoStream", r"Object"),
    (r"WeldJoint", r"Joint"),
    (r"WeldJoint", r"Object"),
    (r"WheelJoint", r"Joint"),
    (r"WheelJoint", r"Object"),
    (r"World", r"Object"),
];

const TYPE_ITEMS: &[LoveApiTypedItem] = &[
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"evaluate",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:evaluate(t: number): number, number",
            documentation: r"Evaluate Bézier curve at parameter t. The parameter must be between 0 and 1 (inclusive).

This function can be used to move objects along paths or tween parameters. However it should not be used to render the curve, see BezierCurve:render for that purpose.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"getControlPoint",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:getControlPoint(i: number): number, number",
            documentation: r"Get coordinates of the i-th control point. Indices start with 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"getControlPointCount",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:getControlPointCount(): number",
            documentation: r"Get the number of control points in the Bézier curve.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"getDegree",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:getDegree(): number",
            documentation: r"Get degree of the Bézier curve. The degree is equal to number-of-control-points - 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"getDerivative",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:getDerivative(): BezierCurve",
            documentation: r"Get the derivative of the Bézier curve.

This function can be used to rotate sprites moving along a curve in the direction of the movement and compute the direction perpendicular to the curve at some parameter t.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"getSegment",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:getSegment(startpoint: number, endpoint: number): BezierCurve",
            documentation: r"Gets a BezierCurve that corresponds to the specified segment of this BezierCurve.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"insertControlPoint",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:insertControlPoint(x: number, y: number, i?: number)",
            documentation: r"Insert control point as the new i-th control point. Existing control points from i onwards are pushed back by 1. Indices start with 1. Negative indices wrap around: -1 is the last control point, -2 the one before the last, etc.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"removeControlPoint",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:removeControlPoint(index: number)",
            documentation: r"Removes the specified control point.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"render",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:render(depth?: number): table",
            documentation: r"Get a list of coordinates to be used with love.graphics.line.

This function samples the Bézier curve using recursive subdivision. You can control the recursion depth using the depth parameter.

If you are just interested to know the position on the curve given a parameter, use BezierCurve:evaluate.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"renderSegment",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:renderSegment(startpoint: number, endpoint: number, depth?: number): table",
            documentation: r"Get a list of coordinates on a specific part of the curve, to be used with love.graphics.line.

This function samples the Bézier curve using recursive subdivision. You can control the recursion depth using the depth parameter.

If you are just need to know the position on the curve given a parameter, use BezierCurve:evaluate.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"rotate",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:rotate(angle: number, ox?: number, oy?: number)",
            documentation: r"Rotate the Bézier curve by an angle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"scale",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:scale(s: number, ox?: number, oy?: number)",
            documentation: r"Scale the Bézier curve by a factor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"setControlPoint",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:setControlPoint(i: number, x: number, y: number)",
            documentation: r"Set coordinates of the i-th control point. Indices start with 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"translate",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:translate(dx: number, dy: number)",
            documentation: r"Move the Bézier curve by an offset.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BezierCurve",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function BezierCurve:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"applyAngularImpulse",
            kind: LuaItemKind::Function,
            detail: r"function Body:applyAngularImpulse(impulse: number)",
            documentation: r"Applies an angular impulse to a body. This makes a single, instantaneous addition to the body momentum.

A body with with a larger mass will react less. The reaction does '''not''' depend on the timestep, and is equivalent to applying a force continuously for 1 second. Impulses are best used to give a single push to a body. For a continuous push to a body it is better to use Body:applyForce.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"applyForce",
            kind: LuaItemKind::Function,
            detail: r"function Body:applyForce(fx: number, fy: number)",
            documentation: r"Apply force to a Body.

A force pushes a body in a direction. A body with with a larger mass will react less. The reaction also depends on how long a force is applied: since the force acts continuously over the entire timestep, a short timestep will only push the body for a short time. Thus forces are best used for many timesteps to give a continuous push to a body (like gravity). For a single push that is independent of timestep, it is better to use Body:applyLinearImpulse.

If the position to apply the force is not given, it will act on the center of mass of the body. The part of the force not directed towards the center of mass will cause the body to spin (and depends on the rotational inertia).

Note that the force components and position must be given in world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"applyLinearImpulse",
            kind: LuaItemKind::Function,
            detail: r"function Body:applyLinearImpulse(ix: number, iy: number)",
            documentation: r"Applies an impulse to a body.

This makes a single, instantaneous addition to the body momentum.

An impulse pushes a body in a direction. A body with with a larger mass will react less. The reaction does '''not''' depend on the timestep, and is equivalent to applying a force continuously for 1 second. Impulses are best used to give a single push to a body. For a continuous push to a body it is better to use Body:applyForce.

If the position to apply the impulse is not given, it will act on the center of mass of the body. The part of the impulse not directed towards the center of mass will cause the body to spin (and depends on the rotational inertia). 

Note that the impulse components and position must be given in world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"applyTorque",
            kind: LuaItemKind::Function,
            detail: r"function Body:applyTorque(torque: number)",
            documentation: r"Apply torque to a body.

Torque is like a force that will change the angular velocity (spin) of a body. The effect will depend on the rotational inertia a body has.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function Body:destroy()",
            documentation: r"Explicitly destroys the Body and all fixtures and joints attached to it.

An error will occur if you attempt to use the object after calling this function. In 0.7.2, when you don't have time to wait for garbage collection, this function may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getAngle",
            kind: LuaItemKind::Function,
            detail: r"function Body:getAngle(): number",
            documentation: r"Get the angle of the body.

The angle is measured in radians. If you need to transform it to degrees, use math.deg.

A value of 0 radians will mean 'looking to the right'. Although radians increase counter-clockwise, the y axis points down so it becomes ''clockwise'' from our point of view.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getAngularDamping",
            kind: LuaItemKind::Function,
            detail: r"function Body:getAngularDamping(): number",
            documentation: r"Gets the Angular damping of the Body

The angular damping is the ''rate of decrease of the angular velocity over time'': A spinning body with no damping and no external forces will continue spinning indefinitely. A spinning body with damping will gradually stop spinning.

Damping is not the same as friction - they can be modelled together. However, only damping is provided by Box2D (and LOVE).

Damping parameters should be between 0 and infinity, with 0 meaning no damping, and infinity meaning full damping. Normally you will use a damping value between 0 and 0.1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getAngularVelocity",
            kind: LuaItemKind::Function,
            detail: r"function Body:getAngularVelocity(): number",
            documentation: r"Get the angular velocity of the Body.

The angular velocity is the ''rate of change of angle over time''.

It is changed in World:update by applying torques, off centre forces/impulses, and angular damping. It can be set directly with Body:setAngularVelocity.

If you need the ''rate of change of position over time'', use Body:getLinearVelocity.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getContacts",
            kind: LuaItemKind::Function,
            detail: r"function Body:getContacts(): table",
            documentation: r"Gets a list of all Contacts attached to the Body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getFixtures",
            kind: LuaItemKind::Function,
            detail: r"function Body:getFixtures(): table",
            documentation: r"Returns a table with all fixtures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getGravityScale",
            kind: LuaItemKind::Function,
            detail: r"function Body:getGravityScale(): number",
            documentation: r"Returns the gravity scale factor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getInertia",
            kind: LuaItemKind::Function,
            detail: r"function Body:getInertia(): number",
            documentation: r"Gets the rotational inertia of the body.

The rotational inertia is how hard is it to make the body spin.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getJoints",
            kind: LuaItemKind::Function,
            detail: r"function Body:getJoints(): table",
            documentation: r"Returns a table containing the Joints attached to this Body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getLinearDamping",
            kind: LuaItemKind::Function,
            detail: r"function Body:getLinearDamping(): number",
            documentation: r"Gets the linear damping of the Body.

The linear damping is the ''rate of decrease of the linear velocity over time''. A moving body with no damping and no external forces will continue moving indefinitely, as is the case in space. A moving body with damping will gradually stop moving.

Damping is not the same as friction - they can be modelled together.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getLinearVelocity",
            kind: LuaItemKind::Function,
            detail: r"function Body:getLinearVelocity(): number, number",
            documentation: r"Gets the linear velocity of the Body from its center of mass.

The linear velocity is the ''rate of change of position over time''.

If you need the ''rate of change of angle over time'', use Body:getAngularVelocity.

If you need to get the linear velocity of a point different from the center of mass:

*  Body:getLinearVelocityFromLocalPoint allows you to specify the point in local coordinates.

*  Body:getLinearVelocityFromWorldPoint allows you to specify the point in world coordinates.

See page 136 of 'Essential Mathematics for Games and Interactive Applications' for definitions of local and world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getLinearVelocityFromLocalPoint",
            kind: LuaItemKind::Function,
            detail: r"function Body:getLinearVelocityFromLocalPoint(x: number, y: number): number, number",
            documentation: r"Get the linear velocity of a point on the body.

The linear velocity for a point on the body is the velocity of the body center of mass plus the velocity at that point from the body spinning.

The point on the body must given in local coordinates. Use Body:getLinearVelocityFromWorldPoint to specify this with world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getLinearVelocityFromWorldPoint",
            kind: LuaItemKind::Function,
            detail: r"function Body:getLinearVelocityFromWorldPoint(x: number, y: number): number, number",
            documentation: r"Get the linear velocity of a point on the body.

The linear velocity for a point on the body is the velocity of the body center of mass plus the velocity at that point from the body spinning.

The point on the body must given in world coordinates. Use Body:getLinearVelocityFromLocalPoint to specify this with local coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getLocalCenter",
            kind: LuaItemKind::Function,
            detail: r"function Body:getLocalCenter(): number, number",
            documentation: r"Get the center of mass position in local coordinates.

Use Body:getWorldCenter to get the center of mass in world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getLocalPoint",
            kind: LuaItemKind::Function,
            detail: r"function Body:getLocalPoint(worldX: number, worldY: number): number, number",
            documentation: r"Transform a point from world coordinates to local coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getLocalPoints",
            kind: LuaItemKind::Function,
            detail: r"function Body:getLocalPoints(x1: number, y1: number, x2: number, y2: number, ...): number, number, number, number, number",
            documentation: r"Transforms multiple points from world coordinates to local coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getLocalVector",
            kind: LuaItemKind::Function,
            detail: r"function Body:getLocalVector(worldX: number, worldY: number): number, number",
            documentation: r"Transform a vector from world coordinates to local coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getMass",
            kind: LuaItemKind::Function,
            detail: r"function Body:getMass(): number",
            documentation: r"Get the mass of the body.

Static bodies always have a mass of 0.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getMassData",
            kind: LuaItemKind::Function,
            detail: r"function Body:getMassData(): number, number, number, number",
            documentation: r"Returns the mass, its center, and the rotational inertia.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getPosition",
            kind: LuaItemKind::Function,
            detail: r"function Body:getPosition(): number, number",
            documentation: r"Get the position of the body.

Note that this may not be the center of mass of the body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getTransform",
            kind: LuaItemKind::Function,
            detail: r"function Body:getTransform(): number, number, number",
            documentation: r"Get the position and angle of the body.

Note that the position may not be the center of mass of the body. An angle of 0 radians will mean 'looking to the right'. Although radians increase counter-clockwise, the y axis points down so it becomes clockwise from our point of view.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function Body:getType(): BodyType",
            documentation: r"Returns the type of the body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function Body:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getWorld",
            kind: LuaItemKind::Function,
            detail: r"function Body:getWorld(): World",
            documentation: r"Gets the World the body lives in.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getWorldCenter",
            kind: LuaItemKind::Function,
            detail: r"function Body:getWorldCenter(): number, number",
            documentation: r"Get the center of mass position in world coordinates.

Use Body:getLocalCenter to get the center of mass in local coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getWorldPoint",
            kind: LuaItemKind::Function,
            detail: r"function Body:getWorldPoint(localX: number, localY: number): number, number",
            documentation: r"Transform a point from local coordinates to world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getWorldPoints",
            kind: LuaItemKind::Function,
            detail: r"function Body:getWorldPoints(x1: number, y1: number, x2: number, y2: number): number, number, number, number",
            documentation: r"Transforms multiple points from local coordinates to world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getWorldVector",
            kind: LuaItemKind::Function,
            detail: r"function Body:getWorldVector(localX: number, localY: number): number, number",
            documentation: r"Transform a vector from local coordinates to world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getX",
            kind: LuaItemKind::Function,
            detail: r"function Body:getX(): number",
            documentation: r"Get the x position of the body in world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"getY",
            kind: LuaItemKind::Function,
            detail: r"function Body:getY(): number",
            documentation: r"Get the y position of the body in world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"isActive",
            kind: LuaItemKind::Function,
            detail: r"function Body:isActive(): boolean",
            documentation: r"Returns whether the body is actively used in the simulation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"isAwake",
            kind: LuaItemKind::Function,
            detail: r"function Body:isAwake(): boolean",
            documentation: r"Returns the sleep status of the body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"isBullet",
            kind: LuaItemKind::Function,
            detail: r"function Body:isBullet(): boolean",
            documentation: r"Get the bullet status of a body.

There are two methods to check for body collisions:

*  at their location when the world is updated (default)

*  using continuous collision detection (CCD)

The default method is efficient, but a body moving very quickly may sometimes jump over another body without producing a collision. A body that is set as a bullet will use CCD. This is less efficient, but is guaranteed not to jump when moving quickly.

Note that static bodies (with zero mass) always use CCD, so your walls will not let a fast moving body pass through even if it is not a bullet.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function Body:isDestroyed(): boolean",
            documentation: r"Gets whether the Body is destroyed. Destroyed bodies cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"isFixedRotation",
            kind: LuaItemKind::Function,
            detail: r"function Body:isFixedRotation(): boolean",
            documentation: r"Returns whether the body rotation is locked.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"isSleepingAllowed",
            kind: LuaItemKind::Function,
            detail: r"function Body:isSleepingAllowed(): boolean",
            documentation: r"Returns the sleeping behaviour of the body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"isTouching",
            kind: LuaItemKind::Function,
            detail: r"function Body:isTouching(otherbody: Body): boolean",
            documentation: r"Gets whether the Body is touching the given other Body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"resetMassData",
            kind: LuaItemKind::Function,
            detail: r"function Body:resetMassData()",
            documentation: r"Resets the mass of the body by recalculating it from the mass properties of the fixtures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setActive",
            kind: LuaItemKind::Function,
            detail: r"function Body:setActive(active: boolean)",
            documentation: r"Sets whether the body is active in the world.

An inactive body does not take part in the simulation. It will not move or cause any collisions.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setAngle",
            kind: LuaItemKind::Function,
            detail: r"function Body:setAngle(angle: number)",
            documentation: r"Set the angle of the body.

The angle is measured in radians. If you need to transform it from degrees, use math.rad.

A value of 0 radians will mean 'looking to the right'. Although radians increase counter-clockwise, the y axis points down so it becomes ''clockwise'' from our point of view.

It is possible to cause a collision with another body by changing its angle. ",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setAngularDamping",
            kind: LuaItemKind::Function,
            detail: r"function Body:setAngularDamping(damping: number)",
            documentation: r"Sets the angular damping of a Body

See Body:getAngularDamping for a definition of angular damping.

Angular damping can take any value from 0 to infinity. It is recommended to stay between 0 and 0.1, though. Other values will look unrealistic.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setAngularVelocity",
            kind: LuaItemKind::Function,
            detail: r"function Body:setAngularVelocity(w: number)",
            documentation: r"Sets the angular velocity of a Body.

The angular velocity is the ''rate of change of angle over time''.

This function will not accumulate anything; any impulses previously applied since the last call to World:update will be lost. ",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setAwake",
            kind: LuaItemKind::Function,
            detail: r"function Body:setAwake(awake: boolean)",
            documentation: r"Wakes the body up or puts it to sleep.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setBullet",
            kind: LuaItemKind::Function,
            detail: r"function Body:setBullet(status: boolean)",
            documentation: r"Set the bullet status of a body.

There are two methods to check for body collisions:

*  at their location when the world is updated (default)

*  using continuous collision detection (CCD)

The default method is efficient, but a body moving very quickly may sometimes jump over another body without producing a collision. A body that is set as a bullet will use CCD. This is less efficient, but is guaranteed not to jump when moving quickly.

Note that static bodies (with zero mass) always use CCD, so your walls will not let a fast moving body pass through even if it is not a bullet.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setFixedRotation",
            kind: LuaItemKind::Function,
            detail: r"function Body:setFixedRotation(isFixed: boolean)",
            documentation: r"Set whether a body has fixed rotation.

Bodies with fixed rotation don't vary the speed at which they rotate. Calling this function causes the mass to be reset. ",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setGravityScale",
            kind: LuaItemKind::Function,
            detail: r"function Body:setGravityScale(scale: number)",
            documentation: r"Sets a new gravity scale factor for the body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setInertia",
            kind: LuaItemKind::Function,
            detail: r"function Body:setInertia(inertia: number)",
            documentation: r"Set the inertia of a body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setLinearDamping",
            kind: LuaItemKind::Function,
            detail: r"function Body:setLinearDamping(ld: number)",
            documentation: r"Sets the linear damping of a Body

See Body:getLinearDamping for a definition of linear damping.

Linear damping can take any value from 0 to infinity. It is recommended to stay between 0 and 0.1, though. Other values will make the objects look 'floaty'(if gravity is enabled).",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setLinearVelocity",
            kind: LuaItemKind::Function,
            detail: r"function Body:setLinearVelocity(x: number, y: number)",
            documentation: r"Sets a new linear velocity for the Body.

This function will not accumulate anything; any impulses previously applied since the last call to World:update will be lost.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setMass",
            kind: LuaItemKind::Function,
            detail: r"function Body:setMass(mass: number)",
            documentation: r"Sets a new body mass.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setMassData",
            kind: LuaItemKind::Function,
            detail: r"function Body:setMassData(x: number, y: number, mass: number, inertia: number)",
            documentation: r"Overrides the calculated mass data.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setPosition",
            kind: LuaItemKind::Function,
            detail: r"function Body:setPosition(x: number, y: number)",
            documentation: r"Set the position of the body.

Note that this may not be the center of mass of the body.

This function cannot wake up the body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setSleepingAllowed",
            kind: LuaItemKind::Function,
            detail: r"function Body:setSleepingAllowed(allowed: boolean)",
            documentation: r"Sets the sleeping behaviour of the body. Should sleeping be allowed, a body at rest will automatically sleep. A sleeping body is not simulated unless it collided with an awake body. Be wary that one can end up with a situation like a floating sleeping body if the floor was removed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setTransform",
            kind: LuaItemKind::Function,
            detail: r"function Body:setTransform(x: number, y: number, angle: number)",
            documentation: r"Set the position and angle of the body.

Note that the position may not be the center of mass of the body. An angle of 0 radians will mean 'looking to the right'. Although radians increase counter-clockwise, the y axis points down so it becomes clockwise from our point of view.

This function cannot wake up the body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setType",
            kind: LuaItemKind::Function,
            detail: r"function Body:setType(type: BodyType)",
            documentation: r"Sets a new body type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function Body:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Body.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setX",
            kind: LuaItemKind::Function,
            detail: r"function Body:setX(x: number)",
            documentation: r"Set the x position of the body.

This function cannot wake up the body. ",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"setY",
            kind: LuaItemKind::Function,
            detail: r"function Body:setY(y: number)",
            documentation: r"Set the y position of the body.

This function cannot wake up the body. ",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Body:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Body:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Body",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Body:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ByteData",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function ByteData:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ByteData",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function ByteData:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ByteData",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function ByteData:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ByteData",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function ByteData:clone(): Data",
            documentation: r"Creates a new copy of the Data object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ByteData",
        item: LuaItem {
            label: r"getFFIPointer",
            kind: LuaItemKind::Function,
            detail: r"function ByteData:getFFIPointer(): cdata",
            documentation: r"Gets an FFI pointer to the Data.

This function should be preferred instead of Data:getPointer because the latter uses light userdata which can't store more all possible memory addresses on some new ARM64 architectures, when LuaJIT is used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ByteData",
        item: LuaItem {
            label: r"getPointer",
            kind: LuaItemKind::Function,
            detail: r"function ByteData:getPointer(): light userdata",
            documentation: r"Gets a pointer to the Data. Can be used with libraries such as LuaJIT's FFI.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ByteData",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function ByteData:getSize(): number",
            documentation: r"Gets the Data's size in bytes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ByteData",
        item: LuaItem {
            label: r"getString",
            kind: LuaItemKind::Function,
            detail: r"function ByteData:getString(): string",
            documentation: r"Gets the full Data as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"generateMipmaps",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:generateMipmaps()",
            documentation: r"Generates mipmaps for the Canvas, based on the contents of the highest-resolution mipmap level.

The Canvas must be created with mipmaps set to a MipmapMode other than 'none' for this function to work. It should only be called while the Canvas is not the active render target.

If the mipmap mode is set to 'auto', this function is automatically called inside love.graphics.setCanvas when switching from this Canvas to another Canvas or to the main screen.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getMSAA",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getMSAA(): number",
            documentation: r"Gets the number of multisample antialiasing (MSAA) samples used when drawing to the Canvas.

This may be different than the number used as an argument to love.graphics.newCanvas if the system running LÖVE doesn't support that number.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getMipmapMode",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getMipmapMode(): MipmapMode",
            documentation: r"Gets the MipmapMode this Canvas was created with.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"newImageData",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:newImageData(): ImageData",
            documentation: r"Generates ImageData from the contents of the Canvas.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"renderTo",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:renderTo(func: function, ...)",
            documentation: r"Render to the Canvas using a function.

This is a shortcut to love.graphics.setCanvas:

canvas:renderTo( func )

is the same as

love.graphics.setCanvas( canvas )

func()

love.graphics.setCanvas()",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getDPIScale",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getDPIScale(): number",
            documentation: r"Gets the DPI scale factor of the Texture.

The DPI scale factor represents relative pixel density. A DPI scale factor of 2 means the texture has twice the pixel density in each dimension (4 times as many pixels in the same area) compared to a texture with a DPI scale factor of 1.

For example, a texture with pixel dimensions of 100x100 with a DPI scale factor of 2 will be drawn as if it was 50x50. This is useful with high-dpi /  retina displays to easily allow swapping out higher or lower pixel density Images and Canvases without needing any extra manual scaling logic.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getDepth",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getDepth(): number",
            documentation: r"Gets the depth of a Volume Texture. Returns 1 for 2D, Cubemap, and Array textures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getDepthSampleMode",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getDepthSampleMode(): CompareMode",
            documentation: r"Gets the comparison mode used when sampling from a depth texture in a shader.

Depth texture comparison modes are advanced low-level functionality typically used with shadow mapping in 3D.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getDimensions(): number, number",
            documentation: r"Gets the width and height of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getFilter",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getFilter(): FilterMode, FilterMode, number",
            documentation: r"Gets the filter mode of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getFormat",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getFormat(): PixelFormat",
            documentation: r"Gets the pixel format of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getHeight(): number",
            documentation: r"Gets the height of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getLayerCount",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getLayerCount(): number",
            documentation: r"Gets the number of layers / slices in an Array Texture. Returns 1 for 2D, Cubemap, and Volume textures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getMipmapCount",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getMipmapCount(): number",
            documentation: r"Gets the number of mipmaps contained in the Texture. If the texture was not created with mipmaps, it will return 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getMipmapFilter",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getMipmapFilter(): FilterMode, number",
            documentation: r"Gets the mipmap filter mode for a Texture. Prior to 11.0 this method only worked on Images.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getPixelDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getPixelDimensions(): number, number",
            documentation: r"Gets the width and height in pixels of the Texture.

Texture:getDimensions gets the dimensions of the texture in units scaled by the texture's DPI scale factor, rather than pixels. Use getDimensions for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelDimensions only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getPixelHeight",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getPixelHeight(): number",
            documentation: r"Gets the height in pixels of the Texture.

DPI scale factor, rather than pixels. Use getHeight for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelHeight only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getPixelWidth",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getPixelWidth(): number",
            documentation: r"Gets the width in pixels of the Texture.

DPI scale factor, rather than pixels. Use getWidth for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelWidth only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getTextureType",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getTextureType(): TextureType",
            documentation: r"Gets the type of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getWidth(): number",
            documentation: r"Gets the width of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"getWrap",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:getWrap(): WrapMode, WrapMode, WrapMode",
            documentation: r"Gets the wrapping properties of a Texture.

This function returns the currently set horizontal and vertical wrapping modes for the texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"isReadable",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:isReadable(): boolean",
            documentation: r"Gets whether the Texture can be drawn and sent to a Shader.

Canvases created with stencil and/or depth PixelFormats are not readable by default, unless readable=true is specified in the settings table passed into love.graphics.newCanvas.

Non-readable Canvases can still be rendered to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"setDepthSampleMode",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:setDepthSampleMode(compare: CompareMode)",
            documentation: r"Sets the comparison mode used when sampling from a depth texture in a shader. Depth texture comparison modes are advanced low-level functionality typically used with shadow mapping in 3D.

When using a depth texture with a comparison mode set in a shader, it must be declared as a sampler2DShadow and used in a GLSL 3 Shader. The result of accessing the texture in the shader will return a float between 0 and 1, proportional to the number of samples (up to 4 samples will be used if bilinear filtering is enabled) that passed the test set by the comparison operation.

Depth texture comparison can only be used with readable depth-formatted Canvases.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"setFilter",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)",
            documentation: r"Sets the filter mode of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"setMipmapFilter",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:setMipmapFilter(filtermode: FilterMode, sharpness?: number)",
            documentation: r"Sets the mipmap filter mode for a Texture. Prior to 11.0 this method only worked on Images.

Mipmapping is useful when drawing a texture at a reduced scale. It can improve performance and reduce aliasing issues.

In created with the mipmaps flag enabled for the mipmap filter to have any effect. In versions prior to 0.10.0 it's best to call this method directly after creating the image with love.graphics.newImage, to avoid bugs in certain graphics drivers.

Due to hardware restrictions and driver bugs, in versions prior to 0.10.0 images that weren't loaded from a CompressedData must have power-of-two dimensions (64x64, 512x256, etc.) to use mipmaps.

On mobile devices (Android and iOS), the sharpness parameter is not supported and will do nothing. You can use a custom compressed and its CompressedData has mipmap data included, it will use that.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"setWrap",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:setWrap(horiz: WrapMode, vert?: WrapMode, depth?: WrapMode)",
            documentation: r"Sets the wrapping properties of a Texture.

This function sets the way a Texture is repeated when it is drawn with a Quad that is larger than the texture's extent, or when a custom Shader is used which uses texture coordinates outside of [0, 1]. A texture may be clamped or set to repeat in both horizontal and vertical directions.

Clamped textures appear only once (with the edges of the texture stretching to fill the extent of the Quad), whereas repeated ones repeat as many times as there is room in the Quad.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Canvas",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Canvas:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getChildEdge",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getChildEdge(index: number): EdgeShape",
            documentation: r"Returns a child of the shape as an EdgeShape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getNextVertex",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getNextVertex(): number, number",
            documentation: r"Gets the vertex that establishes a connection to the next shape.

Setting next and previous ChainShape vertices can help prevent unwanted collisions when a flat shape slides along the edge and moves over to the new shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getPoint",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getPoint(index: number): number, number",
            documentation: r"Returns a point of the shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getPoints",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getPoints(): number, number, number, number",
            documentation: r"Returns all points of the shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getPreviousVertex",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getPreviousVertex(): number, number",
            documentation: r"Gets the vertex that establishes a connection to the previous shape.

Setting next and previous ChainShape vertices can help prevent unwanted collisions when a flat shape slides along the edge and moves over to the new shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getVertexCount",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getVertexCount(): number",
            documentation: r"Returns the number of vertices the shape has.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"setNextVertex",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:setNextVertex(x: number, y: number)",
            documentation: r"Sets a vertex that establishes a connection to the next shape.

This can help prevent unwanted collisions when a flat shape slides along the edge and moves over to the new shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"setPreviousVertex",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:setPreviousVertex(x: number, y: number)",
            documentation: r"Sets a vertex that establishes a connection to the previous shape.

This can help prevent unwanted collisions when a flat shape slides along the edge and moves over to the new shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"computeAABB",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:computeAABB(tx: number, ty: number, tr: number, childIndex?: number): number, number, number, number",
            documentation: r"Returns the points of the bounding box for the transformed shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"computeMass",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:computeMass(density: number): number, number, number, number",
            documentation: r"Computes the mass properties for the shape with the specified density.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getChildCount",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getChildCount(): number",
            documentation: r"Returns the number of children the shape has.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getRadius",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getRadius(): number",
            documentation: r"Gets the radius of the shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:getType(): ShapeType",
            documentation: r"Gets a string representing the Shape.

This function can be useful for conditional debug drawing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"rayCast",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:rayCast(x1: number, y1: number, x2: number, y2: number, maxFraction: number, tx: number, ty: number, tr: number, childIndex?: number): number, number, number",
            documentation: r"Casts a ray against the shape and returns the surface normal vector and the line position where the ray hit. If the ray missed the shape, nil will be returned. The Shape can be transformed to get it into the desired position.

The ray starts on the first point of the input line and goes towards the second point of the line. The fourth argument is the maximum distance the ray is going to travel as a scale factor of the input line length.

The childIndex parameter is used to specify which child of a parent shape, such as a ChainShape, will be ray casted. For ChainShapes, the index of 1 is the first edge on the chain. Ray casting a parent shape will only test the child specified so if you want to test every shape of the parent, you must loop through all of its children.

The world position of the impact can be calculated by multiplying the line vector with the third return value and adding it to the line starting point.

hitx, hity = x1 + (x2 - x1) * fraction, y1 + (y2 - y1) * fraction",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"testPoint",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:testPoint(tx: number, ty: number, tr: number, x: number, y: number): boolean",
            documentation: r"This is particularly useful for mouse interaction with the shapes. By looping through all shapes and testing the mouse position with this function, we can find which shapes the mouse touches.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ChainShape",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function ChainShape:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"clear",
            kind: LuaItemKind::Function,
            detail: r"function Channel:clear()",
            documentation: r"Clears all the messages in the Channel queue.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"demand",
            kind: LuaItemKind::Function,
            detail: r"function Channel:demand(): Variant",
            documentation: r"Retrieves the value of a Channel message and removes it from the message queue.

It waits until a message is in the queue then returns the message value.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"getCount",
            kind: LuaItemKind::Function,
            detail: r"function Channel:getCount(): number",
            documentation: r"Retrieves the number of messages in the thread Channel queue.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"hasRead",
            kind: LuaItemKind::Function,
            detail: r"function Channel:hasRead(id: number): boolean",
            documentation: r"Gets whether a pushed value has been popped or otherwise removed from the Channel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"peek",
            kind: LuaItemKind::Function,
            detail: r"function Channel:peek(): Variant",
            documentation: r"Retrieves the value of a Channel message, but leaves it in the queue.

It returns nil if there's no message in the queue.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"performAtomic",
            kind: LuaItemKind::Function,
            detail: r"function Channel:performAtomic(func: function, ...): any, any",
            documentation: r"Executes the specified function atomically with respect to this Channel.

Calling multiple methods in a row on the same Channel is often useful. However if multiple Threads are calling this Channel's methods at the same time, the different calls on each Thread might end up interleaved (e.g. one or more of the second thread's calls may happen in between the first thread's calls.)

This method avoids that issue by making sure the Thread calling the method has exclusive access to the Channel until the specified function has returned.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"pop",
            kind: LuaItemKind::Function,
            detail: r"function Channel:pop(): Variant",
            documentation: r"Retrieves the value of a Channel message and removes it from the message queue.

It returns nil if there are no messages in the queue.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"push",
            kind: LuaItemKind::Function,
            detail: r"function Channel:push(value: Variant): number",
            documentation: r"Send a message to the thread Channel.

See Variant for the list of supported types.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"supply",
            kind: LuaItemKind::Function,
            detail: r"function Channel:supply(value: Variant): boolean",
            documentation: r"Send a message to the thread Channel and wait for a thread to accept it.

See Variant for the list of supported types.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Channel:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Channel:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Channel",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Channel:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"getPoint",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:getPoint(): number, number",
            documentation: r"Gets the center point of the circle shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"getRadius",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:getRadius(): number",
            documentation: r"Gets the radius of the circle shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"setPoint",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:setPoint(x: number, y: number)",
            documentation: r"Sets the location of the center of the circle shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"setRadius",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:setRadius(radius: number)",
            documentation: r"Sets the radius of the circle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"computeAABB",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:computeAABB(tx: number, ty: number, tr: number, childIndex?: number): number, number, number, number",
            documentation: r"Returns the points of the bounding box for the transformed shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"computeMass",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:computeMass(density: number): number, number, number, number",
            documentation: r"Computes the mass properties for the shape with the specified density.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"getChildCount",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:getChildCount(): number",
            documentation: r"Returns the number of children the shape has.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:getType(): ShapeType",
            documentation: r"Gets a string representing the Shape.

This function can be useful for conditional debug drawing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"rayCast",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:rayCast(x1: number, y1: number, x2: number, y2: number, maxFraction: number, tx: number, ty: number, tr: number, childIndex?: number): number, number, number",
            documentation: r"Casts a ray against the shape and returns the surface normal vector and the line position where the ray hit. If the ray missed the shape, nil will be returned. The Shape can be transformed to get it into the desired position.

The ray starts on the first point of the input line and goes towards the second point of the line. The fourth argument is the maximum distance the ray is going to travel as a scale factor of the input line length.

The childIndex parameter is used to specify which child of a parent shape, such as a ChainShape, will be ray casted. For ChainShapes, the index of 1 is the first edge on the chain. Ray casting a parent shape will only test the child specified so if you want to test every shape of the parent, you must loop through all of its children.

The world position of the impact can be calculated by multiplying the line vector with the third return value and adding it to the line starting point.

hitx, hity = x1 + (x2 - x1) * fraction, y1 + (y2 - y1) * fraction",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"testPoint",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:testPoint(tx: number, ty: number, tr: number, x: number, y: number): boolean",
            documentation: r"This is particularly useful for mouse interaction with the shapes. By looping through all shapes and testing the mouse position with this function, we can find which shapes the mouse touches.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CircleShape",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function CircleShape:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"getFormat",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:getFormat(): CompressedDataFormat",
            documentation: r"Gets the compression format of the CompressedData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:clone(): Data",
            documentation: r"Creates a new copy of the Data object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"getFFIPointer",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:getFFIPointer(): cdata",
            documentation: r"Gets an FFI pointer to the Data.

This function should be preferred instead of Data:getPointer because the latter uses light userdata which can't store more all possible memory addresses on some new ARM64 architectures, when LuaJIT is used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"getPointer",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:getPointer(): light userdata",
            documentation: r"Gets a pointer to the Data. Can be used with libraries such as LuaJIT's FFI.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:getSize(): number",
            documentation: r"Gets the Data's size in bytes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"getString",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:getString(): string",
            documentation: r"Gets the full Data as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedData",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function CompressedData:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getDimensions",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getDimensions(): number, number",
            documentation: r"Gets the width and height of the CompressedImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getFormat",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getFormat(): CompressedImageFormat",
            documentation: r"Gets the format of the CompressedImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getHeight(): number",
            documentation: r"Gets the height of the CompressedImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getMipmapCount",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getMipmapCount(): number",
            documentation: r"Gets the number of mipmap levels in the CompressedImageData. The base mipmap level (original image) is included in the count.

Mipmap filtering cannot be activated for an Image:setMipmapFilter will error. Most tools which can create compressed textures are able to automatically generate mipmaps for them in the same file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getWidth(): number",
            documentation: r"Gets the width of the CompressedImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:clone(): Data",
            documentation: r"Creates a new copy of the Data object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getFFIPointer",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getFFIPointer(): cdata",
            documentation: r"Gets an FFI pointer to the Data.

This function should be preferred instead of Data:getPointer because the latter uses light userdata which can't store more all possible memory addresses on some new ARM64 architectures, when LuaJIT is used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getPointer",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getPointer(): light userdata",
            documentation: r"Gets a pointer to the Data. Can be used with libraries such as LuaJIT's FFI.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getSize(): number",
            documentation: r"Gets the Data's size in bytes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"getString",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:getString(): string",
            documentation: r"Gets the full Data as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageData",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function CompressedImageData:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"getChildren",
            kind: LuaItemKind::Function,
            detail: r"function Contact:getChildren(): number, number",
            documentation: r"Gets the child indices of the shapes of the two colliding fixtures. For ChainShapes, an index of 1 is the first edge in the chain.
Used together with Fixture:rayCast or ChainShape:getChildEdge.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"getFixtures",
            kind: LuaItemKind::Function,
            detail: r"function Contact:getFixtures(): Fixture, Fixture",
            documentation: r"Gets the two Fixtures that hold the shapes that are in contact.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"getFriction",
            kind: LuaItemKind::Function,
            detail: r"function Contact:getFriction(): number",
            documentation: r"Get the friction between two shapes that are in contact.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"getNormal",
            kind: LuaItemKind::Function,
            detail: r"function Contact:getNormal(): number, number",
            documentation: r"Get the normal vector between two shapes that are in contact.

This function returns the coordinates of a unit vector that points from the first shape to the second.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"getPositions",
            kind: LuaItemKind::Function,
            detail: r"function Contact:getPositions(): number, number, number, number",
            documentation: r"Returns the contact points of the two colliding fixtures. There can be one or two points.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"getRestitution",
            kind: LuaItemKind::Function,
            detail: r"function Contact:getRestitution(): number",
            documentation: r"Get the restitution between two shapes that are in contact.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"isEnabled",
            kind: LuaItemKind::Function,
            detail: r"function Contact:isEnabled(): boolean",
            documentation: r"Returns whether the contact is enabled. The collision will be ignored if a contact gets disabled in the preSolve callback.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"isTouching",
            kind: LuaItemKind::Function,
            detail: r"function Contact:isTouching(): boolean",
            documentation: r"Returns whether the two colliding fixtures are touching each other.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"resetFriction",
            kind: LuaItemKind::Function,
            detail: r"function Contact:resetFriction()",
            documentation: r"Resets the contact friction to the mixture value of both fixtures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"resetRestitution",
            kind: LuaItemKind::Function,
            detail: r"function Contact:resetRestitution()",
            documentation: r"Resets the contact restitution to the mixture value of both fixtures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"setEnabled",
            kind: LuaItemKind::Function,
            detail: r"function Contact:setEnabled(enabled: boolean)",
            documentation: r"Enables or disables the contact.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"setFriction",
            kind: LuaItemKind::Function,
            detail: r"function Contact:setFriction(friction: number)",
            documentation: r"Sets the contact friction.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"setRestitution",
            kind: LuaItemKind::Function,
            detail: r"function Contact:setRestitution(restitution: number)",
            documentation: r"Sets the contact restitution.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Contact:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Contact:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Contact",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Contact:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Cursor",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function Cursor:getType(): CursorType",
            documentation: r"Gets the type of the Cursor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Cursor",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Cursor:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Cursor",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Cursor:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Cursor",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Cursor:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Data",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function Data:clone(): Data",
            documentation: r"Creates a new copy of the Data object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Data",
        item: LuaItem {
            label: r"getFFIPointer",
            kind: LuaItemKind::Function,
            detail: r"function Data:getFFIPointer(): cdata",
            documentation: r"Gets an FFI pointer to the Data.

This function should be preferred instead of Data:getPointer because the latter uses light userdata which can't store more all possible memory addresses on some new ARM64 architectures, when LuaJIT is used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Data",
        item: LuaItem {
            label: r"getPointer",
            kind: LuaItemKind::Function,
            detail: r"function Data:getPointer(): light userdata",
            documentation: r"Gets a pointer to the Data. Can be used with libraries such as LuaJIT's FFI.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Data",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function Data:getSize(): number",
            documentation: r"Gets the Data's size in bytes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Data",
        item: LuaItem {
            label: r"getString",
            kind: LuaItemKind::Function,
            detail: r"function Data:getString(): string",
            documentation: r"Gets the full Data as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Data",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Data:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Data",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Data:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Data",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Data:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:clone(): Decoder",
            documentation: r"Creates a new copy of current decoder.

The new decoder will start decoding from the beginning of the audio stream.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"decode",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:decode(): SoundData",
            documentation: r"Decodes the audio and returns a SoundData object containing the decoded audio data.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"getBitDepth",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:getBitDepth(): number",
            documentation: r"Returns the number of bits per sample.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"getChannelCount",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:getChannelCount(): number",
            documentation: r"Returns the number of channels in the stream.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"getDuration",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:getDuration(): number",
            documentation: r"Gets the duration of the sound file. It may not always be sample-accurate, and it may return -1 if the duration cannot be determined at all.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"getSampleRate",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:getSampleRate(): number",
            documentation: r"Returns the sample rate of the Decoder.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"seek",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:seek(offset: number)",
            documentation: r"Sets the currently playing position of the Decoder.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Decoder",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Decoder:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getDampingRatio",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getDampingRatio(): number",
            documentation: r"Gets the damping ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getFrequency",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getFrequency(): number",
            documentation: r"Gets the response speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getLength",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getLength(): number",
            documentation: r"Gets the equilibrium distance between the two Bodies.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"setDampingRatio",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:setDampingRatio(ratio: number)",
            documentation: r"Sets the damping ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"setFrequency",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:setFrequency(Hz: number)",
            documentation: r"Sets the response speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"setLength",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:setLength(l: number)",
            documentation: r"Sets the equilibrium distance between the two Bodies.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function DistanceJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Drawable",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Drawable:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Drawable",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Drawable:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Drawable",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Drawable:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"close",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:close(): boolean",
            documentation: r"Closes a File.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"flush",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:flush(): boolean, string",
            documentation: r"Flushes any buffered written data in the file to the disk.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"getBuffer",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:getBuffer(): BufferMode, number",
            documentation: r"Gets the buffer mode of a file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"getFilename",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:getFilename(): string",
            documentation: r"Gets the filename that the File object was created with. If the file object originated from the love.filedropped callback, the filename will be the full platform-dependent file path.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"getMode",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:getMode(): FileMode",
            documentation: r"Gets the FileMode the file has been opened with.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:getSize(): number",
            documentation: r"Returns the file size.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"isEOF",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:isEOF(): boolean",
            documentation: r"Gets whether end-of-file has been reached.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"isOpen",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:isOpen(): boolean",
            documentation: r"Gets whether the file is open.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"lines",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:lines(): function",
            documentation: r"Iterate over all the lines in a file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"open",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:open(mode: FileMode): boolean, string",
            documentation: r"Open the file for write, read or append.

If you are getting the error message 'Could not set write directory', try setting the save directory. This is done either with love.filesystem.setIdentity or by setting the identity field in love.conf (only available with love 0.7 or higher).",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"read",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:read(bytes?: number): string, number",
            documentation: r"Read a number of bytes from a file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"seek",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:seek(pos: number): boolean",
            documentation: r"Seek to a position in a file",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"setBuffer",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:setBuffer(mode: BufferMode, size?: number): boolean, string",
            documentation: r"Sets the buffer mode for a file opened for writing or appending. Files with buffering enabled will not write data to the disk until the buffer size limit is reached, depending on the buffer mode.

File:flush will force any buffered data to be written to the disk.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"tell",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:tell(): number",
            documentation: r"Returns the position in the file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"write",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:write(data: string, size?: number): boolean, string",
            documentation: r"Write data to a file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DroppedFile",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function DroppedFile:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"getNextVertex",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:getNextVertex(): number, number",
            documentation: r"Gets the vertex that establishes a connection to the next shape.

Setting next and previous EdgeShape vertices can help prevent unwanted collisions when a flat shape slides along the edge and moves over to the new shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"getPoints",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:getPoints(): number, number, number, number",
            documentation: r"Returns the local coordinates of the edge points.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"getPreviousVertex",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:getPreviousVertex(): number, number",
            documentation: r"Gets the vertex that establishes a connection to the previous shape.

Setting next and previous EdgeShape vertices can help prevent unwanted collisions when a flat shape slides along the edge and moves over to the new shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"setNextVertex",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:setNextVertex(x: number, y: number)",
            documentation: r"Sets a vertex that establishes a connection to the next shape.

This can help prevent unwanted collisions when a flat shape slides along the edge and moves over to the new shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"setPreviousVertex",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:setPreviousVertex(x: number, y: number)",
            documentation: r"Sets a vertex that establishes a connection to the previous shape.

This can help prevent unwanted collisions when a flat shape slides along the edge and moves over to the new shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"computeAABB",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:computeAABB(tx: number, ty: number, tr: number, childIndex?: number): number, number, number, number",
            documentation: r"Returns the points of the bounding box for the transformed shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"computeMass",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:computeMass(density: number): number, number, number, number",
            documentation: r"Computes the mass properties for the shape with the specified density.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"getChildCount",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:getChildCount(): number",
            documentation: r"Returns the number of children the shape has.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"getRadius",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:getRadius(): number",
            documentation: r"Gets the radius of the shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:getType(): ShapeType",
            documentation: r"Gets a string representing the Shape.

This function can be useful for conditional debug drawing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"rayCast",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:rayCast(x1: number, y1: number, x2: number, y2: number, maxFraction: number, tx: number, ty: number, tr: number, childIndex?: number): number, number, number",
            documentation: r"Casts a ray against the shape and returns the surface normal vector and the line position where the ray hit. If the ray missed the shape, nil will be returned. The Shape can be transformed to get it into the desired position.

The ray starts on the first point of the input line and goes towards the second point of the line. The fourth argument is the maximum distance the ray is going to travel as a scale factor of the input line length.

The childIndex parameter is used to specify which child of a parent shape, such as a ChainShape, will be ray casted. For ChainShapes, the index of 1 is the first edge on the chain. Ray casting a parent shape will only test the child specified so if you want to test every shape of the parent, you must loop through all of its children.

The world position of the impact can be calculated by multiplying the line vector with the third return value and adding it to the line starting point.

hitx, hity = x1 + (x2 - x1) * fraction, y1 + (y2 - y1) * fraction",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"testPoint",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:testPoint(tx: number, ty: number, tr: number, x: number, y: number): boolean",
            documentation: r"This is particularly useful for mouse interaction with the shapes. By looping through all shapes and testing the mouse position with this function, we can find which shapes the mouse touches.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EdgeShape",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function EdgeShape:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"close",
            kind: LuaItemKind::Function,
            detail: r"function File:close(): boolean",
            documentation: r"Closes a File.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"flush",
            kind: LuaItemKind::Function,
            detail: r"function File:flush(): boolean, string",
            documentation: r"Flushes any buffered written data in the file to the disk.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"getBuffer",
            kind: LuaItemKind::Function,
            detail: r"function File:getBuffer(): BufferMode, number",
            documentation: r"Gets the buffer mode of a file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"getFilename",
            kind: LuaItemKind::Function,
            detail: r"function File:getFilename(): string",
            documentation: r"Gets the filename that the File object was created with. If the file object originated from the love.filedropped callback, the filename will be the full platform-dependent file path.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"getMode",
            kind: LuaItemKind::Function,
            detail: r"function File:getMode(): FileMode",
            documentation: r"Gets the FileMode the file has been opened with.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function File:getSize(): number",
            documentation: r"Returns the file size.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"isEOF",
            kind: LuaItemKind::Function,
            detail: r"function File:isEOF(): boolean",
            documentation: r"Gets whether end-of-file has been reached.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"isOpen",
            kind: LuaItemKind::Function,
            detail: r"function File:isOpen(): boolean",
            documentation: r"Gets whether the file is open.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"lines",
            kind: LuaItemKind::Function,
            detail: r"function File:lines(): function",
            documentation: r"Iterate over all the lines in a file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"open",
            kind: LuaItemKind::Function,
            detail: r"function File:open(mode: FileMode): boolean, string",
            documentation: r"Open the file for write, read or append.

If you are getting the error message 'Could not set write directory', try setting the save directory. This is done either with love.filesystem.setIdentity or by setting the identity field in love.conf (only available with love 0.7 or higher).",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"read",
            kind: LuaItemKind::Function,
            detail: r"function File:read(bytes?: number): string, number",
            documentation: r"Read a number of bytes from a file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"seek",
            kind: LuaItemKind::Function,
            detail: r"function File:seek(pos: number): boolean",
            documentation: r"Seek to a position in a file",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"setBuffer",
            kind: LuaItemKind::Function,
            detail: r"function File:setBuffer(mode: BufferMode, size?: number): boolean, string",
            documentation: r"Sets the buffer mode for a file opened for writing or appending. Files with buffering enabled will not write data to the disk until the buffer size limit is reached, depending on the buffer mode.

File:flush will force any buffered data to be written to the disk.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"tell",
            kind: LuaItemKind::Function,
            detail: r"function File:tell(): number",
            documentation: r"Returns the position in the file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"write",
            kind: LuaItemKind::Function,
            detail: r"function File:write(data: string, size?: number): boolean, string",
            documentation: r"Write data to a file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function File:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function File:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"File",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function File:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"getExtension",
            kind: LuaItemKind::Function,
            detail: r"function FileData:getExtension(): string",
            documentation: r"Gets the extension of the FileData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"getFilename",
            kind: LuaItemKind::Function,
            detail: r"function FileData:getFilename(): string",
            documentation: r"Gets the filename of the FileData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function FileData:clone(): Data",
            documentation: r"Creates a new copy of the Data object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"getFFIPointer",
            kind: LuaItemKind::Function,
            detail: r"function FileData:getFFIPointer(): cdata",
            documentation: r"Gets an FFI pointer to the Data.

This function should be preferred instead of Data:getPointer because the latter uses light userdata which can't store more all possible memory addresses on some new ARM64 architectures, when LuaJIT is used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"getPointer",
            kind: LuaItemKind::Function,
            detail: r"function FileData:getPointer(): light userdata",
            documentation: r"Gets a pointer to the Data. Can be used with libraries such as LuaJIT's FFI.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function FileData:getSize(): number",
            documentation: r"Gets the Data's size in bytes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"getString",
            kind: LuaItemKind::Function,
            detail: r"function FileData:getString(): string",
            documentation: r"Gets the full Data as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function FileData:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function FileData:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileData",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function FileData:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:destroy()",
            documentation: r"Destroys the fixture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getBody",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getBody(): Body",
            documentation: r"Returns the body to which the fixture is attached.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getBoundingBox",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getBoundingBox(index?: number): number, number, number, number",
            documentation: r"Returns the points of the fixture bounding box. In case the fixture has multiple children a 1-based index can be specified. For example, a fixture will have multiple children with a chain shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getCategory",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getCategory(): number",
            documentation: r"Returns the categories the fixture belongs to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getDensity",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getDensity(): number",
            documentation: r"Returns the density of the fixture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getFilterData",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getFilterData(): number, number, number",
            documentation: r"Returns the filter data of the fixture.

Categories and masks are encoded as the bits of a 16-bit integer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getFriction",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getFriction(): number",
            documentation: r"Returns the friction of the fixture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getGroupIndex",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getGroupIndex(): number",
            documentation: r"Returns the group the fixture belongs to. Fixtures with the same group will always collide if the group is positive or never collide if it's negative. The group zero means no group.

The groups range from -32768 to 32767.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getMask",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getMask(): number",
            documentation: r"Returns which categories this fixture should '''NOT''' collide with.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getMassData",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getMassData(): number, number, number, number",
            documentation: r"Returns the mass, its center and the rotational inertia.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getRestitution",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getRestitution(): number",
            documentation: r"Returns the restitution of the fixture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getShape",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getShape(): Shape",
            documentation: r"Returns the shape of the fixture. This shape is a reference to the actual data used in the simulation. It's possible to change its values between timesteps.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:getUserData(): any",
            documentation: r"Returns the Lua value associated with this fixture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:isDestroyed(): boolean",
            documentation: r"Gets whether the Fixture is destroyed. Destroyed fixtures cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"isSensor",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:isSensor(): boolean",
            documentation: r"Returns whether the fixture is a sensor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"rayCast",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:rayCast(x1: number, y1: number, x2: number, y2: number, maxFraction: number, childIndex?: number): number, number, number",
            documentation: r"Casts a ray against the shape of the fixture and returns the surface normal vector and the line position where the ray hit. If the ray missed the shape, nil will be returned.

The ray starts on the first point of the input line and goes towards the second point of the line. The fifth argument is the maximum distance the ray is going to travel as a scale factor of the input line length.

The childIndex parameter is used to specify which child of a parent shape, such as a ChainShape, will be ray casted. For ChainShapes, the index of 1 is the first edge on the chain. Ray casting a parent shape will only test the child specified so if you want to test every shape of the parent, you must loop through all of its children.

The world position of the impact can be calculated by multiplying the line vector with the third return value and adding it to the line starting point.

hitx, hity = x1 + (x2 - x1) * fraction, y1 + (y2 - y1) * fraction",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setCategory",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setCategory(...)",
            documentation: r"Sets the categories the fixture belongs to. There can be up to 16 categories represented as a number from 1 to 16.

All fixture's default category is 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setDensity",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setDensity(density: number)",
            documentation: r"Sets the density of the fixture. Call Body:resetMassData if this needs to take effect immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setFilterData",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setFilterData(categories: number, mask: number, group: number)",
            documentation: r"Sets the filter data of the fixture.

Groups, categories, and mask can be used to define the collision behaviour of the fixture.

If two fixtures are in the same group they either always collide if the group is positive, or never collide if it's negative. If the group is zero or they do not match, then the contact filter checks if the fixtures select a category of the other fixture with their masks. The fixtures do not collide if that's not the case. If they do have each other's categories selected, the return value of the custom contact filter will be used. They always collide if none was set.

There can be up to 16 categories. Categories and masks are encoded as the bits of a 16-bit integer.

When created, prior to calling this function, all fixtures have category set to 1, mask set to 65535 (all categories) and group set to 0.

This function allows setting all filter data for a fixture at once. To set only the categories, the mask or the group, you can use Fixture:setCategory, Fixture:setMask or Fixture:setGroupIndex respectively.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setFriction",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setFriction(friction: number)",
            documentation: r"Sets the friction of the fixture.

Friction determines how shapes react when they 'slide' along other shapes. Low friction indicates a slippery surface, like ice, while high friction indicates a rough surface, like concrete. Range: 0.0 - 1.0.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setGroupIndex",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setGroupIndex(group: number)",
            documentation: r"Sets the group the fixture belongs to. Fixtures with the same group will always collide if the group is positive or never collide if it's negative. The group zero means no group.

The groups range from -32768 to 32767.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setMask",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setMask(...)",
            documentation: r"Sets the category mask of the fixture. There can be up to 16 categories represented as a number from 1 to 16.

This fixture will '''NOT''' collide with the fixtures that are in the selected categories if the other fixture also has a category of this fixture selected.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setRestitution",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setRestitution(restitution: number)",
            documentation: r"Sets the restitution of the fixture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setSensor",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setSensor(sensor: boolean)",
            documentation: r"Sets whether the fixture should act as a sensor.

Sensors do not cause collision responses, but the begin-contact and end-contact World callbacks will still be called for this fixture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:setUserData(value: any)",
            documentation: r"Associates a Lua value with the fixture.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"testPoint",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:testPoint(x: number, y: number): boolean",
            documentation: r"Checks if a point is inside the shape of the fixture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Fixture",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Fixture:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getAscent",
            kind: LuaItemKind::Function,
            detail: r"function Font:getAscent(): number",
            documentation: r"Gets the ascent of the Font.

The ascent spans the distance between the baseline and the top of the glyph that reaches farthest from the baseline.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getBaseline",
            kind: LuaItemKind::Function,
            detail: r"function Font:getBaseline(): number",
            documentation: r"Gets the baseline of the Font.

Most scripts share the notion of a baseline: an imaginary horizontal line on which characters rest. In some scripts, parts of glyphs lie below the baseline.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getDPIScale",
            kind: LuaItemKind::Function,
            detail: r"function Font:getDPIScale(): number",
            documentation: r"Gets the DPI scale factor of the Font.

The DPI scale factor represents relative pixel density. A DPI scale factor of 2 means the font's glyphs have twice the pixel density in each dimension (4 times as many pixels in the same area) compared to a font with a DPI scale factor of 1.

The font size of TrueType fonts is scaled internally by the font's specified DPI scale factor. By default, LÖVE uses the screen's DPI scale factor when creating TrueType fonts.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getDescent",
            kind: LuaItemKind::Function,
            detail: r"function Font:getDescent(): number",
            documentation: r"Gets the descent of the Font.

The descent spans the distance between the baseline and the lowest descending glyph in a typeface.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getFilter",
            kind: LuaItemKind::Function,
            detail: r"function Font:getFilter(): FilterMode, FilterMode, number",
            documentation: r"Gets the filter mode for a font.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function Font:getHeight(): number",
            documentation: r"Gets the height of the Font.

The height of the font is the size including any spacing; the height which it will need.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getKerning",
            kind: LuaItemKind::Function,
            detail: r"function Font:getKerning(leftchar: string, rightchar: string): number",
            documentation: r"Gets the kerning between two characters in the Font.

Kerning is normally handled automatically in love.graphics.print, Text objects, Font:getWidth, Font:getWrap, etc. This function is useful when stitching text together manually.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getLineHeight",
            kind: LuaItemKind::Function,
            detail: r"function Font:getLineHeight(): number",
            documentation: r"Gets the line height.

This will be the value previously set by Font:setLineHeight, or 1.0 by default.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function Font:getWidth(text: string): number",
            documentation: r"Determines the maximum width (accounting for newlines) taken by the given string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"getWrap",
            kind: LuaItemKind::Function,
            detail: r"function Font:getWrap(text: string, wraplimit: number): number, table",
            documentation: r"Gets formatting information for text, given a wrap limit.

This function accounts for newlines correctly (i.e. '\n').",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"hasGlyphs",
            kind: LuaItemKind::Function,
            detail: r"function Font:hasGlyphs(text: string): boolean",
            documentation: r"Gets whether the Font can render a character or string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"setFallbacks",
            kind: LuaItemKind::Function,
            detail: r"function Font:setFallbacks(fallbackfont1: Font, ...)",
            documentation: r"Sets the fallback fonts. When the Font doesn't contain a glyph, it will substitute the glyph from the next subsequent fallback Fonts. This is akin to setting a 'font stack' in Cascading Style Sheets (CSS).

If this is called it should be before love.graphics.print, Font:getWrap, and other Font methods which use glyph positioning information are called.

Every fallback Font must be created from the same file type as the primary Font. For example, a Font created from a .ttf file can only use fallback Fonts that were created from .ttf files.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"setFilter",
            kind: LuaItemKind::Function,
            detail: r"function Font:setFilter(min: FilterMode, mag: FilterMode, anisotropy?: number)",
            documentation: r"Sets the filter mode for a font.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"setLineHeight",
            kind: LuaItemKind::Function,
            detail: r"function Font:setLineHeight(height: number)",
            documentation: r"Sets the line height.

When rendering the font in lines the actual height will be determined by the line height multiplied by the height of the font. The default is 1.0.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Font:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Font:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Font",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Font:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getMaxForce",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getMaxForce(): number",
            documentation: r"Gets the maximum friction force in Newtons.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getMaxTorque",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getMaxTorque(): number",
            documentation: r"Gets the maximum friction torque in Newton-meters.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"setMaxForce",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:setMaxForce(maxForce: number)",
            documentation: r"Sets the maximum friction force in Newtons.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"setMaxTorque",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:setMaxTorque(torque: number)",
            documentation: r"Sets the maximum friction torque in Newton-meters.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FrictionJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function FrictionJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getJoints",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getJoints(): Joint, Joint",
            documentation: r"Get the Joints connected by this GearJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getRatio",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getRatio(): number",
            documentation: r"Get the ratio of a gear joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"setRatio",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:setRatio(ratio: number)",
            documentation: r"Set the ratio of a gear joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GearJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function GearJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getAdvance",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getAdvance(): number",
            documentation: r"Gets glyph advance.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getBearing",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getBearing(): number, number",
            documentation: r"Gets glyph bearing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getBoundingBox",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getBoundingBox(): number, number, number, number",
            documentation: r"Gets glyph bounding box.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getDimensions",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getDimensions(): number, number",
            documentation: r"Gets glyph dimensions.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getFormat",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getFormat(): PixelFormat",
            documentation: r"Gets glyph pixel format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getGlyph",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getGlyph(): number",
            documentation: r"Gets glyph number.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getGlyphString",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getGlyphString(): string",
            documentation: r"Gets glyph string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getHeight(): number",
            documentation: r"Gets glyph height.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getWidth(): number",
            documentation: r"Gets glyph width.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:clone(): Data",
            documentation: r"Creates a new copy of the Data object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getFFIPointer",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getFFIPointer(): cdata",
            documentation: r"Gets an FFI pointer to the Data.

This function should be preferred instead of Data:getPointer because the latter uses light userdata which can't store more all possible memory addresses on some new ARM64 architectures, when LuaJIT is used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getPointer",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getPointer(): light userdata",
            documentation: r"Gets a pointer to the Data. Can be used with libraries such as LuaJIT's FFI.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getSize(): number",
            documentation: r"Gets the Data's size in bytes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"getString",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:getString(): string",
            documentation: r"Gets the full Data as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GlyphData",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function GlyphData:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"isCompressed",
            kind: LuaItemKind::Function,
            detail: r"function Image:isCompressed(): boolean",
            documentation: r"Gets whether the Image was created from CompressedData.

Compressed images take up less space in VRAM, and drawing a compressed image will generally be more efficient than drawing one created from raw pixel data.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"isFormatLinear",
            kind: LuaItemKind::Function,
            detail: r"function Image:isFormatLinear(): boolean",
            documentation: r"Gets whether the Image was created with the linear (non-gamma corrected) flag set to true.

This method always returns false when gamma-correct rendering is not enabled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"replacePixels",
            kind: LuaItemKind::Function,
            detail: r"function Image:replacePixels(data: ImageData, slice?: number, mipmap?: number, x?: number, y?: number, reloadmipmaps?: boolean)",
            documentation: r"Replace the contents of an Image.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getDPIScale",
            kind: LuaItemKind::Function,
            detail: r"function Image:getDPIScale(): number",
            documentation: r"Gets the DPI scale factor of the Texture.

The DPI scale factor represents relative pixel density. A DPI scale factor of 2 means the texture has twice the pixel density in each dimension (4 times as many pixels in the same area) compared to a texture with a DPI scale factor of 1.

For example, a texture with pixel dimensions of 100x100 with a DPI scale factor of 2 will be drawn as if it was 50x50. This is useful with high-dpi /  retina displays to easily allow swapping out higher or lower pixel density Images and Canvases without needing any extra manual scaling logic.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getDepth",
            kind: LuaItemKind::Function,
            detail: r"function Image:getDepth(): number",
            documentation: r"Gets the depth of a Volume Texture. Returns 1 for 2D, Cubemap, and Array textures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getDepthSampleMode",
            kind: LuaItemKind::Function,
            detail: r"function Image:getDepthSampleMode(): CompareMode",
            documentation: r"Gets the comparison mode used when sampling from a depth texture in a shader.

Depth texture comparison modes are advanced low-level functionality typically used with shadow mapping in 3D.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Image:getDimensions(): number, number",
            documentation: r"Gets the width and height of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getFilter",
            kind: LuaItemKind::Function,
            detail: r"function Image:getFilter(): FilterMode, FilterMode, number",
            documentation: r"Gets the filter mode of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getFormat",
            kind: LuaItemKind::Function,
            detail: r"function Image:getFormat(): PixelFormat",
            documentation: r"Gets the pixel format of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function Image:getHeight(): number",
            documentation: r"Gets the height of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getLayerCount",
            kind: LuaItemKind::Function,
            detail: r"function Image:getLayerCount(): number",
            documentation: r"Gets the number of layers / slices in an Array Texture. Returns 1 for 2D, Cubemap, and Volume textures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getMipmapCount",
            kind: LuaItemKind::Function,
            detail: r"function Image:getMipmapCount(): number",
            documentation: r"Gets the number of mipmaps contained in the Texture. If the texture was not created with mipmaps, it will return 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getMipmapFilter",
            kind: LuaItemKind::Function,
            detail: r"function Image:getMipmapFilter(): FilterMode, number",
            documentation: r"Gets the mipmap filter mode for a Texture. Prior to 11.0 this method only worked on Images.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getPixelDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Image:getPixelDimensions(): number, number",
            documentation: r"Gets the width and height in pixels of the Texture.

Texture:getDimensions gets the dimensions of the texture in units scaled by the texture's DPI scale factor, rather than pixels. Use getDimensions for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelDimensions only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getPixelHeight",
            kind: LuaItemKind::Function,
            detail: r"function Image:getPixelHeight(): number",
            documentation: r"Gets the height in pixels of the Texture.

DPI scale factor, rather than pixels. Use getHeight for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelHeight only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getPixelWidth",
            kind: LuaItemKind::Function,
            detail: r"function Image:getPixelWidth(): number",
            documentation: r"Gets the width in pixels of the Texture.

DPI scale factor, rather than pixels. Use getWidth for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelWidth only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getTextureType",
            kind: LuaItemKind::Function,
            detail: r"function Image:getTextureType(): TextureType",
            documentation: r"Gets the type of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function Image:getWidth(): number",
            documentation: r"Gets the width of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"getWrap",
            kind: LuaItemKind::Function,
            detail: r"function Image:getWrap(): WrapMode, WrapMode, WrapMode",
            documentation: r"Gets the wrapping properties of a Texture.

This function returns the currently set horizontal and vertical wrapping modes for the texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"isReadable",
            kind: LuaItemKind::Function,
            detail: r"function Image:isReadable(): boolean",
            documentation: r"Gets whether the Texture can be drawn and sent to a Shader.

Canvases created with stencil and/or depth PixelFormats are not readable by default, unless readable=true is specified in the settings table passed into love.graphics.newCanvas.

Non-readable Canvases can still be rendered to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"setDepthSampleMode",
            kind: LuaItemKind::Function,
            detail: r"function Image:setDepthSampleMode(compare: CompareMode)",
            documentation: r"Sets the comparison mode used when sampling from a depth texture in a shader. Depth texture comparison modes are advanced low-level functionality typically used with shadow mapping in 3D.

When using a depth texture with a comparison mode set in a shader, it must be declared as a sampler2DShadow and used in a GLSL 3 Shader. The result of accessing the texture in the shader will return a float between 0 and 1, proportional to the number of samples (up to 4 samples will be used if bilinear filtering is enabled) that passed the test set by the comparison operation.

Depth texture comparison can only be used with readable depth-formatted Canvases.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"setFilter",
            kind: LuaItemKind::Function,
            detail: r"function Image:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)",
            documentation: r"Sets the filter mode of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"setMipmapFilter",
            kind: LuaItemKind::Function,
            detail: r"function Image:setMipmapFilter(filtermode: FilterMode, sharpness?: number)",
            documentation: r"Sets the mipmap filter mode for a Texture. Prior to 11.0 this method only worked on Images.

Mipmapping is useful when drawing a texture at a reduced scale. It can improve performance and reduce aliasing issues.

In created with the mipmaps flag enabled for the mipmap filter to have any effect. In versions prior to 0.10.0 it's best to call this method directly after creating the image with love.graphics.newImage, to avoid bugs in certain graphics drivers.

Due to hardware restrictions and driver bugs, in versions prior to 0.10.0 images that weren't loaded from a CompressedData must have power-of-two dimensions (64x64, 512x256, etc.) to use mipmaps.

On mobile devices (Android and iOS), the sharpness parameter is not supported and will do nothing. You can use a custom compressed and its CompressedData has mipmap data included, it will use that.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"setWrap",
            kind: LuaItemKind::Function,
            detail: r"function Image:setWrap(horiz: WrapMode, vert?: WrapMode, depth?: WrapMode)",
            documentation: r"Sets the wrapping properties of a Texture.

This function sets the way a Texture is repeated when it is drawn with a Quad that is larger than the texture's extent, or when a custom Shader is used which uses texture coordinates outside of [0, 1]. A texture may be clamped or set to repeat in both horizontal and vertical directions.

Clamped textures appear only once (with the edges of the texture stretching to fill the extent of the Quad), whereas repeated ones repeat as many times as there is room in the Quad.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Image:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Image:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Image",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Image:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"encode",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:encode(format: ImageFormat, filename?: string): FileData",
            documentation: r"Encodes the ImageData and optionally writes it to the save directory.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getDimensions",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getDimensions(): number, number",
            documentation: r"Gets the width and height of the ImageData in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getHeight(): number",
            documentation: r"Gets the height of the ImageData in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getPixel",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getPixel(x: number, y: number): number, number, number, number",
            documentation: r"Gets the color of a pixel at a specific position in the image.

Valid x and y values start at 0 and go up to image width and height minus 1. Non-integer values are floored.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getWidth(): number",
            documentation: r"Gets the width of the ImageData in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"mapPixel",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:mapPixel(pixelFunction: function, x?: number, y?: number, width?: number, height?: number)",
            documentation: r"Transform an image by applying a function to every pixel.

This function is a higher-order function. It takes another function as a parameter, and calls it once for each pixel in the ImageData.

The passed function is called with six parameters for each pixel in turn. The parameters are numbers that represent the x and y coordinates of the pixel and its red, green, blue and alpha values. The function should return the new red, green, blue, and alpha values for that pixel.

function pixelFunction(x, y, r, g, b, a)

    -- template for defining your own pixel mapping function

    -- perform computations giving the new values for r, g, b and a

    -- ...

    return r, g, b, a

end

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"paste",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:paste(source: ImageData, dx: number, dy: number, sx: number, sy: number, sw: number, sh: number)",
            documentation: r"Paste into ImageData from another source ImageData.

Note that this function just replaces the contents in the destination rectangle; it does not do any alpha blending.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"setPixel",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:setPixel(x: number, y: number, r: number, g: number, b: number, a: number)",
            documentation: r"Sets the color of a pixel at a specific position in the image.

Valid x and y values start at 0 and go up to image width and height minus 1.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getFormat",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getFormat(): PixelFormat",
            documentation: r"Gets the pixel format of the ImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:clone(): Data",
            documentation: r"Creates a new copy of the Data object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getFFIPointer",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getFFIPointer(): cdata",
            documentation: r"Gets an FFI pointer to the Data.

This function should be preferred instead of Data:getPointer because the latter uses light userdata which can't store more all possible memory addresses on some new ARM64 architectures, when LuaJIT is used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getPointer",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getPointer(): light userdata",
            documentation: r"Gets a pointer to the Data. Can be used with libraries such as LuaJIT's FFI.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getSize(): number",
            documentation: r"Gets the Data's size in bytes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"getString",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:getString(): string",
            documentation: r"Gets the full Data as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageData",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function ImageData:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function Joint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function Joint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function Joint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function Joint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function Joint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function Joint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function Joint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function Joint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function Joint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function Joint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Joint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Joint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Joint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getAxes",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getAxes(): number, number, number",
            documentation: r"Gets the direction of each axis.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getAxis",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getAxis(axis: number): number",
            documentation: r"Gets the direction of an axis.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getAxisCount",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getAxisCount(): number",
            documentation: r"Gets the number of axes on the joystick.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getButtonCount",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getButtonCount(): number",
            documentation: r"Gets the number of buttons on the joystick.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getDeviceInfo",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getDeviceInfo(): number, number, number",
            documentation: r"Gets the USB vendor ID, product ID, and product version numbers of joystick which consistent across operating systems.

Can be used to show different icons, etc. for different gamepads.

Some Linux distribution may not ship with SDL 2.0.6 or later, in which case this function will returns 0 for all the three values.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getGUID",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getGUID(): string",
            documentation: r"Gets a stable GUID unique to the type of the physical joystick which does not change over time. For example, all Sony Dualshock 3 controllers in OS X have the same GUID. The value is platform-dependent.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getGamepadAxis",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getGamepadAxis(axis: GamepadAxis): number",
            documentation: r"Gets the direction of a virtual gamepad axis. If the Joystick isn't recognized as a gamepad or isn't connected, this function will always return 0.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getGamepadMapping",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getGamepadMapping(axis: GamepadAxis): JoystickInputType, number, JoystickHat",
            documentation: r"Gets the button, axis or hat that a virtual gamepad input is bound to.

Returns nil if the Joystick isn't recognized as a gamepad or the virtual gamepad axis is not bound to a Joystick input.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getGamepadMappingString",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getGamepadMappingString(): string",
            documentation: r"Gets the full gamepad mapping string of this Joystick, or nil if it's not recognized as a gamepad.

The mapping string contains binding information used to map the Joystick's buttons an axes to the standard gamepad layout, and can be used later with love.joystick.loadGamepadMappings.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getHat",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getHat(hat: number): JoystickHat",
            documentation: r"Gets the direction of the Joystick's hat.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getHatCount",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getHatCount(): number",
            documentation: r"Gets the number of hats on the joystick.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getID",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getID(): number, number",
            documentation: r"Gets the joystick's unique identifier. The identifier will remain the same for the life of the game, even when the Joystick is disconnected and reconnected, but it '''will''' change when the game is re-launched.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getName",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getName(): string",
            documentation: r"Gets the name of the joystick.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"getVibration",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:getVibration(): number, number",
            documentation: r"Gets the current vibration motor strengths on a Joystick with rumble support.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"isConnected",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:isConnected(): boolean",
            documentation: r"Gets whether the Joystick is connected.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"isDown",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:isDown(buttonN: number): boolean",
            documentation: r"Checks if a button on the Joystick is pressed.

LÖVE 0.9.0 had a bug which required the button indices passed to Joystick:isDown to be 0-based instead of 1-based, for example button 1 would be 0 for this function. It was fixed in 0.9.1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"isGamepad",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:isGamepad(): boolean",
            documentation: r"Gets whether the Joystick is recognized as a gamepad. If this is the case, the Joystick's buttons and axes can be used in a standardized manner across different operating systems and joystick models via Joystick:getGamepadAxis, Joystick:isGamepadDown, love.gamepadpressed, and related functions.

LÖVE automatically recognizes most popular controllers with a similar layout to the Xbox 360 controller as gamepads, but you can add more with love.joystick.setGamepadMapping.

If the Joystick is recognized as a gamepad, the physical locations for the virtual gamepad axes and buttons correspond as closely as possible to the layout of a standard Xbox 360 controller.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"isGamepadDown",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:isGamepadDown(buttonN: GamepadButton): boolean",
            documentation: r"Checks if a virtual gamepad button on the Joystick is pressed. If the Joystick is not recognized as a Gamepad or isn't connected, then this function will always return false.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"isVibrationSupported",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:isVibrationSupported(): boolean",
            documentation: r"Gets whether the Joystick supports vibration.

The very first call to this function may take more time than expected because SDL's Haptic / Force Feedback subsystem needs to be initialized.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"setVibration",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:setVibration(left: number, right: number): boolean",
            documentation: r"Sets the vibration motor speeds on a Joystick with rumble support. Most common gamepads have this functionality, although not all drivers give proper support. Use Joystick:isVibrationSupported to check.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Joystick",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Joystick:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"attachAttribute",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:attachAttribute(name: string, mesh: Mesh)",
            documentation: r"Attaches a vertex attribute from a different Mesh onto this Mesh, for use when drawing. This can be used to share vertex attribute data between several different Meshes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"detachAttribute",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:detachAttribute(name: string): boolean",
            documentation: r"Removes a previously attached vertex attribute from this Mesh.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"flush",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:flush()",
            documentation: r"Immediately sends all modified vertex data in the Mesh to the graphics card.

Normally it isn't necessary to call this method as love.graphics.draw(mesh, ...) will do it automatically if needed, but explicitly using **Mesh:flush** gives more control over when the work happens.

If this method is used, it generally shouldn't be called more than once (at most) between love.graphics.draw(mesh, ...) calls.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"getDrawMode",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:getDrawMode(): MeshDrawMode",
            documentation: r"Gets the mode used when drawing the Mesh.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"getDrawRange",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:getDrawRange(): number, number",
            documentation: r"Gets the range of vertices used when drawing the Mesh.

If the Mesh's draw range has not been set previously with Mesh:setDrawRange, this function will return nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"getTexture",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:getTexture(): Texture",
            documentation: r"Gets the texture (Image or Canvas) used when drawing the Mesh.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"getVertex",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:getVertex(index: number): number, number",
            documentation: r"Gets the properties of a vertex in the Mesh.

In versions prior to 11.0, color and byte component values were within the range of 0 to 255 instead of 0 to 1.

The values are returned in the same order as the vertex attributes in the Mesh's vertex format. A standard Mesh that wasn't created with a custom vertex format will return two position numbers, two texture coordinate numbers, and four color components: x, y, u, v, r, g, b, a.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"getVertexAttribute",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:getVertexAttribute(vertexindex: number, attributeindex: number): number, number, number",
            documentation: r"Gets the properties of a specific attribute within a vertex in the Mesh.

Meshes without a custom vertex format specified in love.graphics.newMesh have position as their first attribute, texture coordinates as their second attribute, and color as their third attribute.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"getVertexCount",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:getVertexCount(): number",
            documentation: r"Gets the total number of vertices in the Mesh.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"getVertexFormat",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:getVertexFormat(): table",
            documentation: r"Gets the vertex format that the Mesh was created with.

If a Mesh wasn't created with a custom vertex format, it will have the following vertex format:

defaultformat = {

    {'VertexPosition', 'float', 2}, -- The x,y position of each vertex.

    {'VertexTexCoord', 'float', 2}, -- The u,v texture coordinates of each vertex.

    {'VertexColor', 'byte', 4} -- The r,g,b,a color of each vertex.

}",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"getVertexMap",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:getVertexMap(): table",
            documentation: r"Gets the vertex map for the Mesh. The vertex map describes the order in which the vertices are used when the Mesh is drawn. The vertices, vertex map, and mesh draw mode work together to determine what exactly is displayed on the screen.

If no vertex map has been set previously via Mesh:setVertexMap, then this function will return nil in LÖVE 0.10.0+, or an empty table in 0.9.2 and older.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"isAttributeEnabled",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:isAttributeEnabled(name: string): boolean",
            documentation: r"Gets whether a specific vertex attribute in the Mesh is enabled. Vertex data from disabled attributes is not used when drawing the Mesh.

If a Mesh wasn't created with a custom vertex format, it will have 3 vertex attributes named VertexPosition, VertexTexCoord, and VertexColor. Otherwise the attribute name must either match one of the vertex attributes specified in the vertex format when creating the Mesh, 

or must match a vertex attribute from another Mesh attached to this Mesh via Mesh:attachAttribute.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"setAttributeEnabled",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:setAttributeEnabled(name: string, enable: boolean)",
            documentation: r"Enables or disables a specific vertex attribute in the Mesh. Vertex data from disabled attributes is not used when drawing the Mesh.

If a Mesh wasn't created with a custom vertex format, it will have 3 vertex attributes named VertexPosition, VertexTexCoord, and VertexColor. Otherwise the attribute name must either match one of the vertex attributes specified in the vertex format when creating the Mesh, 

or must match a vertex attribute from another Mesh attached to this Mesh via Mesh:attachAttribute.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"setDrawMode",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:setDrawMode(mode: MeshDrawMode)",
            documentation: r"Sets the mode used when drawing the Mesh.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"setDrawRange",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:setDrawRange(start: number, count: number)",
            documentation: r"Restricts the drawn vertices of the Mesh to a subset of the total.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"setTexture",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:setTexture(texture: Texture)",
            documentation: r"Sets the texture (Image or Canvas) used when drawing the Mesh.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"setVertex",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:setVertex(index: number, attributecomponent: number, ...)",
            documentation: r"Sets the properties of a vertex in the Mesh.

In versions prior to 11.0, color and byte component values were within the range of 0 to 255 instead of 0 to 1.

The arguments are in the same order as the vertex attributes in the Mesh's vertex format. A standard Mesh that wasn't created with a custom vertex format will use two position numbers, two texture coordinate numbers, and four color components per vertex: x, y, u, v, r, g, b, a.

If no value is supplied for a specific vertex attribute component, it will be set to a default value of 0 if its data type is 'float', or 1 if its data type is 'byte'.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"setVertexAttribute",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:setVertexAttribute(vertexindex: number, attributeindex: number, value1: number, value2: number, ...)",
            documentation: r"Sets the properties of a specific attribute within a vertex in the Mesh.

Meshes without a custom vertex format specified in love.graphics.newMesh have position as their first attribute, texture coordinates as their second attribute, and color as their third attribute.

Attribute components which exist within the attribute but are not specified as arguments default to 0 for attributes with the float data type, and 255 for the byte data type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"setVertexMap",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:setVertexMap(map: table)",
            documentation: r"Sets the vertex map for the Mesh. The vertex map describes the order in which the vertices are used when the Mesh is drawn. The vertices, vertex map, and mesh draw mode work together to determine what exactly is displayed on the screen.

The vertex map allows you to re-order or reuse vertices when drawing without changing the actual vertex parameters or duplicating vertices. It is especially useful when combined with different Mesh Draw Modes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"setVertices",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:setVertices(vertices: table, startvertex?: number, count?: number)",
            documentation: r"Replaces a range of vertices in the Mesh with new ones. The total number of vertices in a Mesh cannot be changed after it has been created. This is often more efficient than calling Mesh:setVertex in a loop.

The values in each vertex table are in the same order as the vertex attributes in the Mesh's vertex format. A standard Mesh that wasn't created with a custom vertex format will use two position numbers, two texture coordinate numbers, and four color components per vertex: x, y, u, v, r, g, b, a.

If no value is supplied for a specific vertex attribute component, it will be set to a default value of 0 if its data type is 'float', or 255 if its data type is 'byte'.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Mesh",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Mesh:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getAngularOffset",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getAngularOffset(): number",
            documentation: r"Gets the target angular offset between the two Bodies the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getLinearOffset",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getLinearOffset(): number, number",
            documentation: r"Gets the target linear offset between the two Bodies the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"setAngularOffset",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:setAngularOffset(angleoffset: number)",
            documentation: r"Sets the target angluar offset between the two Bodies the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"setLinearOffset",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:setLinearOffset(x: number, y: number)",
            documentation: r"Sets the target linear offset between the two Bodies the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MotorJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function MotorJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getDampingRatio",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getDampingRatio(): number",
            documentation: r"Returns the damping ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getFrequency",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getFrequency(): number",
            documentation: r"Returns the frequency.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getMaxForce",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getMaxForce(): number",
            documentation: r"Gets the highest allowed force.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getTarget",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getTarget(): number, number",
            documentation: r"Gets the target point.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"setDampingRatio",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:setDampingRatio(ratio: number)",
            documentation: r"Sets a new damping ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"setFrequency",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:setFrequency(freq: number)",
            documentation: r"Sets a new frequency.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"setMaxForce",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:setMaxForce(f: number)",
            documentation: r"Sets the highest allowed force.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"setTarget",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:setTarget(x: number, y: number)",
            documentation: r"Sets the target point.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MouseJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function MouseJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Object",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Object:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Object",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Object:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Object",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Object:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:clone(): ParticleSystem",
            documentation: r"Creates an identical copy of the ParticleSystem in the stopped state.

Cloned ParticleSystem inherit all the set-able state of the original ParticleSystem, but they are initialized stopped.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"emit",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:emit(numparticles: number)",
            documentation: r"Emits a burst of particles from the particle emitter.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getBufferSize",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getBufferSize(): number",
            documentation: r"Gets the maximum number of particles the ParticleSystem can have at once.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getColors",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getColors(): number, number, number, number, number, number, number, number, number, number, number, number",
            documentation: r"Gets the series of colors applied to the particle sprite.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getCount",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getCount(): number",
            documentation: r"Gets the number of particles that are currently in the system.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getDirection",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getDirection(): number",
            documentation: r"Gets the direction of the particle emitter (in radians).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getEmissionArea",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getEmissionArea(): AreaSpreadDistribution, number, number, number, boolean",
            documentation: r"Gets the area-based spawn parameters for the particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getEmissionRate",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getEmissionRate(): number",
            documentation: r"Gets the amount of particles emitted per second.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getEmitterLifetime",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getEmitterLifetime(): number",
            documentation: r"Gets how long the particle system will emit particles (if -1 then it emits particles forever).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getInsertMode",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getInsertMode(): ParticleInsertMode",
            documentation: r"Gets the mode used when the ParticleSystem adds new particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getLinearAcceleration",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getLinearAcceleration(): number, number, number, number",
            documentation: r"Gets the linear acceleration (acceleration along the x and y axes) for particles.

Every particle created will accelerate along the x and y axes between xmin,ymin and xmax,ymax.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getLinearDamping",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getLinearDamping(): number, number",
            documentation: r"Gets the amount of linear damping (constant deceleration) for particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getOffset",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getOffset(): number, number",
            documentation: r"Gets the particle image's draw offset.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getParticleLifetime",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getParticleLifetime(): number, number",
            documentation: r"Gets the lifetime of the particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getPosition",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getPosition(): number, number",
            documentation: r"Gets the position of the emitter.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getQuads",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getQuads(): table",
            documentation: r"Gets the series of Quads used for the particle sprites.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getRadialAcceleration",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getRadialAcceleration(): number, number",
            documentation: r"Gets the radial acceleration (away from the emitter).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getRotation",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getRotation(): number, number",
            documentation: r"Gets the rotation of the image upon particle creation (in radians).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getSizeVariation",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getSizeVariation(): number",
            documentation: r"Gets the amount of size variation (0 meaning no variation and 1 meaning full variation between start and end).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getSizes",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getSizes(): number, number, number",
            documentation: r"Gets the series of sizes by which the sprite is scaled. 1.0 is normal size. The particle system will interpolate between each size evenly over the particle's lifetime.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getSpeed",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getSpeed(): number, number",
            documentation: r"Gets the speed of the particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getSpin",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getSpin(): number, number, number",
            documentation: r"Gets the spin of the sprite.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getSpinVariation",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getSpinVariation(): number",
            documentation: r"Gets the amount of spin variation (0 meaning no variation and 1 meaning full variation between start and end).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getSpread",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getSpread(): number",
            documentation: r"Gets the amount of directional spread of the particle emitter (in radians).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getTangentialAcceleration",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getTangentialAcceleration(): number, number",
            documentation: r"Gets the tangential acceleration (acceleration perpendicular to the particle's direction).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"getTexture",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:getTexture(): Texture",
            documentation: r"Gets the texture (Image or Canvas) used for the particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"hasRelativeRotation",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:hasRelativeRotation(): boolean",
            documentation: r"Gets whether particle angles and rotations are relative to their velocities. If enabled, particles are aligned to the angle of their velocities and rotate relative to that angle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"isActive",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:isActive(): boolean",
            documentation: r"Checks whether the particle system is actively emitting particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"isPaused",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:isPaused(): boolean",
            documentation: r"Checks whether the particle system is paused.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"isStopped",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:isStopped(): boolean",
            documentation: r"Checks whether the particle system is stopped.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"moveTo",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:moveTo(x: number, y: number)",
            documentation: r"Moves the position of the emitter. This results in smoother particle spawning behaviour than if ParticleSystem:setPosition is used every frame.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"pause",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:pause()",
            documentation: r"Pauses the particle emitter.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"reset",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:reset()",
            documentation: r"Resets the particle emitter, removing any existing particles and resetting the lifetime counter.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setBufferSize",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setBufferSize(size: number)",
            documentation: r"Sets the size of the buffer (the max allowed amount of particles in the system).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setColors",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setColors(r1: number, g1: number, b1: number, a1?: number, ...)",
            documentation: r"Sets a series of colors to apply to the particle sprite. The particle system will interpolate between each color evenly over the particle's lifetime.

Arguments can be passed in groups of four, representing the components of the desired RGBA value, or as tables of RGBA component values, with a default alpha value of 1 if only three values are given. At least one color must be specified. A maximum of eight may be used.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setDirection",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setDirection(direction: number)",
            documentation: r"Sets the direction the particles will be emitted in.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setEmissionArea",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setEmissionArea(distribution: AreaSpreadDistribution, dx: number, dy: number, angle?: number, directionRelativeToCenter?: boolean)",
            documentation: r"Sets area-based spawn parameters for the particles. Newly created particles will spawn in an area around the emitter based on the parameters to this function.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setEmissionRate",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setEmissionRate(rate: number)",
            documentation: r"Sets the amount of particles emitted per second.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setEmitterLifetime",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setEmitterLifetime(life: number)",
            documentation: r"Sets how long the particle system should emit particles (if -1 then it emits particles forever).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setInsertMode",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setInsertMode(mode: ParticleInsertMode)",
            documentation: r"Sets the mode to use when the ParticleSystem adds new particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setLinearAcceleration",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setLinearAcceleration(xmin: number, ymin: number, xmax?: number, ymax?: number)",
            documentation: r"Sets the linear acceleration (acceleration along the x and y axes) for particles.

Every particle created will accelerate along the x and y axes between xmin,ymin and xmax,ymax.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setLinearDamping",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setLinearDamping(min: number, max?: number)",
            documentation: r"Sets the amount of linear damping (constant deceleration) for particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setOffset",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setOffset(x: number, y: number)",
            documentation: r"Set the offset position which the particle sprite is rotated around.

If this function is not used, the particles rotate around their center.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setParticleLifetime",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setParticleLifetime(min: number, max?: number)",
            documentation: r"Sets the lifetime of the particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setPosition",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setPosition(x: number, y: number)",
            documentation: r"Sets the position of the emitter.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setQuads",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setQuads(quad1: Quad, ...)",
            documentation: r"Sets a series of Quads to use for the particle sprites. Particles will choose a Quad from the list based on the particle's current lifetime, allowing for the use of animated sprite sheets with ParticleSystems.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setRadialAcceleration",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setRadialAcceleration(min: number, max?: number)",
            documentation: r"Set the radial acceleration (away from the emitter).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setRelativeRotation",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setRelativeRotation(enable: boolean)",
            documentation: r"Sets whether particle angles and rotations are relative to their velocities. If enabled, particles are aligned to the angle of their velocities and rotate relative to that angle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setRotation",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setRotation(min: number, max?: number)",
            documentation: r"Sets the rotation of the image upon particle creation (in radians).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setSizeVariation",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setSizeVariation(variation: number)",
            documentation: r"Sets the amount of size variation (0 meaning no variation and 1 meaning full variation between start and end).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setSizes",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setSizes(size1: number, size2?: number, size8?: number)",
            documentation: r"Sets a series of sizes by which to scale a particle sprite. 1.0 is normal size. The particle system will interpolate between each size evenly over the particle's lifetime.

At least one size must be specified. A maximum of eight may be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setSpeed",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setSpeed(min: number, max?: number)",
            documentation: r"Sets the speed of the particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setSpin",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setSpin(min: number, max?: number)",
            documentation: r"Sets the spin of the sprite.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setSpinVariation",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setSpinVariation(variation: number)",
            documentation: r"Sets the amount of spin variation (0 meaning no variation and 1 meaning full variation between start and end).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setSpread",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setSpread(spread: number)",
            documentation: r"Sets the amount of spread for the system.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setTangentialAcceleration",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setTangentialAcceleration(min: number, max?: number)",
            documentation: r"Sets the tangential acceleration (acceleration perpendicular to the particle's direction).",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"setTexture",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:setTexture(texture: Texture)",
            documentation: r"Sets the texture (Image or Canvas) to be used for the particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"start",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:start()",
            documentation: r"Starts the particle emitter.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"stop",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:stop()",
            documentation: r"Stops the particle emitter, resetting the lifetime counter.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"update",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:update(dt: number)",
            documentation: r"Updates the particle system; moving, creating and killing particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleSystem",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function ParticleSystem:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"getPoints",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:getPoints(): number, number, number, number",
            documentation: r"Get the local coordinates of the polygon's vertices.

This function has a variable number of return values. It can be used in a nested fashion with love.graphics.polygon.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"computeAABB",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:computeAABB(tx: number, ty: number, tr: number, childIndex?: number): number, number, number, number",
            documentation: r"Returns the points of the bounding box for the transformed shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"computeMass",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:computeMass(density: number): number, number, number, number",
            documentation: r"Computes the mass properties for the shape with the specified density.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"getChildCount",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:getChildCount(): number",
            documentation: r"Returns the number of children the shape has.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"getRadius",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:getRadius(): number",
            documentation: r"Gets the radius of the shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:getType(): ShapeType",
            documentation: r"Gets a string representing the Shape.

This function can be useful for conditional debug drawing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"rayCast",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:rayCast(x1: number, y1: number, x2: number, y2: number, maxFraction: number, tx: number, ty: number, tr: number, childIndex?: number): number, number, number",
            documentation: r"Casts a ray against the shape and returns the surface normal vector and the line position where the ray hit. If the ray missed the shape, nil will be returned. The Shape can be transformed to get it into the desired position.

The ray starts on the first point of the input line and goes towards the second point of the line. The fourth argument is the maximum distance the ray is going to travel as a scale factor of the input line length.

The childIndex parameter is used to specify which child of a parent shape, such as a ChainShape, will be ray casted. For ChainShapes, the index of 1 is the first edge on the chain. Ray casting a parent shape will only test the child specified so if you want to test every shape of the parent, you must loop through all of its children.

The world position of the impact can be calculated by multiplying the line vector with the third return value and adding it to the line starting point.

hitx, hity = x1 + (x2 - x1) * fraction, y1 + (y2 - y1) * fraction",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"testPoint",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:testPoint(tx: number, ty: number, tr: number, x: number, y: number): boolean",
            documentation: r"This is particularly useful for mouse interaction with the shapes. By looping through all shapes and testing the mouse position with this function, we can find which shapes the mouse touches.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PolygonShape",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function PolygonShape:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"areLimitsEnabled",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:areLimitsEnabled(): boolean",
            documentation: r"Checks whether the limits are enabled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getAxis",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getAxis(): number, number",
            documentation: r"Gets the world-space axis vector of the Prismatic Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getJointSpeed",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getJointSpeed(): number",
            documentation: r"Get the current joint angle speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getJointTranslation",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getJointTranslation(): number",
            documentation: r"Get the current joint translation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getLimits",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getLimits(): number, number",
            documentation: r"Gets the joint limits.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getLowerLimit",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getLowerLimit(): number",
            documentation: r"Gets the lower limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getMaxMotorForce",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getMaxMotorForce(): number",
            documentation: r"Gets the maximum motor force.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getMotorForce",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getMotorForce(invdt: number): number",
            documentation: r"Returns the current motor force.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getMotorSpeed",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getMotorSpeed(): number",
            documentation: r"Gets the motor speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getReferenceAngle",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getReferenceAngle(): number",
            documentation: r"Gets the reference angle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getUpperLimit",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getUpperLimit(): number",
            documentation: r"Gets the upper limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"isMotorEnabled",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:isMotorEnabled(): boolean",
            documentation: r"Checks whether the motor is enabled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"setLimits",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:setLimits(lower: number, upper: number)",
            documentation: r"Sets the limits.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"setLimitsEnabled",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:setLimitsEnabled(): boolean",
            documentation: r"Enables/disables the joint limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"setLowerLimit",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:setLowerLimit(lower: number)",
            documentation: r"Sets the lower limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"setMaxMotorForce",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:setMaxMotorForce(f: number)",
            documentation: r"Set the maximum motor force.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"setMotorEnabled",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:setMotorEnabled(enable: boolean)",
            documentation: r"Enables/disables the joint motor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"setMotorSpeed",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:setMotorSpeed(s: number)",
            documentation: r"Sets the motor speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"setUpperLimit",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:setUpperLimit(upper: number)",
            documentation: r"Sets the upper limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PrismaticJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function PrismaticJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getConstant",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getConstant(): number",
            documentation: r"Get the total length of the rope.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getGroundAnchors",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getGroundAnchors(): number, number, number, number",
            documentation: r"Get the ground anchor positions in world coordinates.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getLengthA",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getLengthA(): number",
            documentation: r"Get the current length of the rope segment attached to the first body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getLengthB",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getLengthB(): number",
            documentation: r"Get the current length of the rope segment attached to the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getMaxLengths",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getMaxLengths(): number, number",
            documentation: r"Get the maximum lengths of the rope segments.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getRatio",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getRatio(): number",
            documentation: r"Get the pulley ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"setConstant",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:setConstant(length: number)",
            documentation: r"Set the total length of the rope.

Setting a new length for the rope updates the maximum length values of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"setMaxLengths",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:setMaxLengths(max1: number, max2: number)",
            documentation: r"Set the maximum lengths of the rope segments.

The physics module also imposes maximum values for the rope segments. If the parameters exceed these values, the maximum values are set instead of the requested values.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"setRatio",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:setRatio(ratio: number)",
            documentation: r"Set the pulley ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PulleyJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function PulleyJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Quad",
        item: LuaItem {
            label: r"getTextureDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Quad:getTextureDimensions(): number, number",
            documentation: r"Gets reference texture dimensions initially specified in love.graphics.newQuad.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Quad",
        item: LuaItem {
            label: r"getViewport",
            kind: LuaItemKind::Function,
            detail: r"function Quad:getViewport(): number, number, number, number",
            documentation: r"Gets the current viewport of this Quad.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Quad",
        item: LuaItem {
            label: r"setViewport",
            kind: LuaItemKind::Function,
            detail: r"function Quad:setViewport(x: number, y: number, w: number, h: number, sw?: number, sh?: number)",
            documentation: r"Sets the texture coordinates according to a viewport.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Quad",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Quad:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Quad",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Quad:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Quad",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Quad:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"getSeed",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:getSeed(): number, number",
            documentation: r"Gets the seed of the random number generator object.

The seed is split into two numbers due to Lua's use of doubles for all number values - doubles can't accurately represent integer  values above 2^53, but the seed value is an integer number in the range of 2^64 - 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"getState",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:getState(): string",
            documentation: r"Gets the current state of the random number generator. This returns an opaque string which is only useful for later use with RandomGenerator:setState in the same major version of LÖVE.

This is different from RandomGenerator:getSeed in that getState gets the RandomGenerator's current state, whereas getSeed gets the previously set seed number.

The value of the state string does not depend on the current operating system.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"random",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:random(): number",
            documentation: r"Generates a pseudo-random number in a platform independent manner.

Get uniformly distributed pseudo-random number within 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"randomNormal",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:randomNormal(stddev?: number, mean?: number): number",
            documentation: r"Get a normally distributed pseudo random number.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"setSeed",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:setSeed(seed: number)",
            documentation: r"Sets the seed of the random number generator using the specified integer number.

Due to Lua's use of double-precision floating point numbers, values above 2^53 cannot be accurately represented. Use the other variant of this function if your seed will have a larger value.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"setState",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:setState(state: string)",
            documentation: r"Sets the current state of the random number generator. The value used as an argument for this function is an opaque string and should only originate from a previous call to RandomGenerator:getState in the same major version of LÖVE.

This is different from RandomGenerator:setSeed in that setState directly sets the RandomGenerator's current implementation-dependent state, whereas setSeed gives it a new seed value.

The effect of the state string does not depend on the current operating system.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RandomGenerator",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function RandomGenerator:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"getAdvance",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:getAdvance(): number",
            documentation: r"Gets font advance.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"getAscent",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:getAscent(): number",
            documentation: r"Gets ascent height.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"getDescent",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:getDescent(): number",
            documentation: r"Gets descent height.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"getGlyphCount",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:getGlyphCount(): number",
            documentation: r"Gets number of glyphs in font.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"getGlyphData",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:getGlyphData(glyph: string): GlyphData",
            documentation: r"Gets glyph data of a specified glyph.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:getHeight(): number",
            documentation: r"Gets font height.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"getLineHeight",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:getLineHeight(): number",
            documentation: r"Gets line height of a font.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"hasGlyphs",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:hasGlyphs(glyph1: string | number, ...): boolean",
            documentation: r"Checks if font contains specified glyphs.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Rasterizer",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Rasterizer:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"getBitDepth",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:getBitDepth(): number",
            documentation: r"Gets the number of bits per sample in the data currently being recorded.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"getChannelCount",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:getChannelCount(): number",
            documentation: r"Gets the number of channels currently being recorded (mono or stereo).",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"getData",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:getData(): SoundData",
            documentation: r"Gets all recorded audio SoundData stored in the device's internal ring buffer.

The internal ring buffer is cleared when this function is called, so calling it again will only get audio recorded after the previous call. If the device's internal ring buffer completely fills up before getData is called, the oldest data that doesn't fit into the buffer will be lost.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"getName",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:getName(): string",
            documentation: r"Gets the name of the recording device.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"getSampleCount",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:getSampleCount(): number",
            documentation: r"Gets the number of currently recorded samples.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"getSampleRate",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:getSampleRate(): number",
            documentation: r"Gets the number of samples per second currently being recorded.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"isRecording",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:isRecording(): boolean",
            documentation: r"Gets whether the device is currently recording.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"start",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:start(samplecount: number, samplerate?: number, bitdepth?: number, channels?: number): boolean",
            documentation: r"Begins recording audio using this device.

A ring buffer is used internally to store recorded data before RecordingDevice:getData or RecordingDevice:stop are called – the former clears the buffer. If the buffer completely fills up before getData or stop are called, the oldest data that doesn't fit into the buffer will be lost.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"stop",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:stop(): SoundData",
            documentation: r"Stops recording audio from this device. Any sound data currently in the device's buffer will be returned.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RecordingDevice",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function RecordingDevice:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"areLimitsEnabled",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:areLimitsEnabled(): boolean",
            documentation: r"Checks whether limits are enabled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getJointAngle",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getJointAngle(): number",
            documentation: r"Get the current joint angle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getJointSpeed",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getJointSpeed(): number",
            documentation: r"Get the current joint angle speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getLimits",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getLimits(): number, number",
            documentation: r"Gets the joint limits.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getLowerLimit",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getLowerLimit(): number",
            documentation: r"Gets the lower limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getMaxMotorTorque",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getMaxMotorTorque(): number",
            documentation: r"Gets the maximum motor force.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getMotorSpeed",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getMotorSpeed(): number",
            documentation: r"Gets the motor speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getMotorTorque",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getMotorTorque(): number",
            documentation: r"Get the current motor force.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getReferenceAngle",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getReferenceAngle(): number",
            documentation: r"Gets the reference angle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getUpperLimit",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getUpperLimit(): number",
            documentation: r"Gets the upper limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"hasLimitsEnabled",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:hasLimitsEnabled(): boolean",
            documentation: r"Checks whether limits are enabled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"isMotorEnabled",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:isMotorEnabled(): boolean",
            documentation: r"Checks whether the motor is enabled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"setLimits",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:setLimits(lower: number, upper: number)",
            documentation: r"Sets the limits.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"setLimitsEnabled",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:setLimitsEnabled(enable: boolean)",
            documentation: r"Enables/disables the joint limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"setLowerLimit",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:setLowerLimit(lower: number)",
            documentation: r"Sets the lower limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"setMaxMotorTorque",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:setMaxMotorTorque(f: number)",
            documentation: r"Set the maximum motor force.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"setMotorEnabled",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:setMotorEnabled(enable: boolean)",
            documentation: r"Enables/disables the joint motor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"setMotorSpeed",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:setMotorSpeed(s: number)",
            documentation: r"Sets the motor speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"setUpperLimit",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:setUpperLimit(upper: number)",
            documentation: r"Sets the upper limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RevoluteJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function RevoluteJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"getMaxLength",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:getMaxLength(): number",
            documentation: r"Gets the maximum length of a RopeJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"setMaxLength",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:setMaxLength(maxLength: number)",
            documentation: r"Sets the maximum length of a RopeJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"RopeJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function RopeJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shader",
        item: LuaItem {
            label: r"getWarnings",
            kind: LuaItemKind::Function,
            detail: r"function Shader:getWarnings(): string",
            documentation: r"Returns any warning and error messages from compiling the shader code. This can be used for debugging your shaders if there's anything the graphics hardware doesn't like.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shader",
        item: LuaItem {
            label: r"hasUniform",
            kind: LuaItemKind::Function,
            detail: r"function Shader:hasUniform(name: string): boolean",
            documentation: r"Gets whether a uniform / extern variable exists in the Shader.

If a graphics driver's shader compiler determines that a uniform / extern variable doesn't affect the final output of the shader, it may optimize the variable out. This function will return false in that case.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shader",
        item: LuaItem {
            label: r"send",
            kind: LuaItemKind::Function,
            detail: r"function Shader:send(name: string, number: number, ...)",
            documentation: r"Sends one or more values to a special (''uniform'') variable inside the shader. Uniform variables have to be marked using the ''uniform'' or ''extern'' keyword, e.g.

uniform float time;  // 'float' is the typical number type used in GLSL shaders.

uniform float varsvec2 light_pos;

uniform vec4 colors[4;

The corresponding send calls would be

shader:send('time', t)

shader:send('vars',a,b)

shader:send('light_pos', {light_x, light_y})

shader:send('colors', {r1, g1, b1, a1},  {r2, g2, b2, a2},  {r3, g3, b3, a3},  {r4, g4, b4, a4})

Uniform / extern variables are read-only in the shader code and remain constant until modified by a Shader:send call. Uniform variables can be accessed in both the Vertex and Pixel components of a shader, as long as the variable is declared in each.

Because all numbers in Lua are floating point, in versions prior to 0.10.2 you must use the function Shader:sendInt to send values to uniform int variables in the shader's code.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shader",
        item: LuaItem {
            label: r"sendColor",
            kind: LuaItemKind::Function,
            detail: r"function Shader:sendColor(name: string, color: table, ...)",
            documentation: r"Sends one or more colors to a special (''extern'' / ''uniform'') vec3 or vec4 variable inside the shader. The color components must be in the range of 1. The colors are gamma-corrected if global gamma-correction is enabled.

Extern variables must be marked using the ''extern'' keyword, e.g.

extern vec4 Color;

The corresponding sendColor call would be

shader:sendColor('Color', {r, g, b, a})

Extern variables can be accessed in both the Vertex and Pixel stages of a shader, as long as the variable is declared in each.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shader",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Shader:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shader",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Shader:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shader",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Shader:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"computeAABB",
            kind: LuaItemKind::Function,
            detail: r"function Shape:computeAABB(tx: number, ty: number, tr: number, childIndex?: number): number, number, number, number",
            documentation: r"Returns the points of the bounding box for the transformed shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"computeMass",
            kind: LuaItemKind::Function,
            detail: r"function Shape:computeMass(density: number): number, number, number, number",
            documentation: r"Computes the mass properties for the shape with the specified density.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"getChildCount",
            kind: LuaItemKind::Function,
            detail: r"function Shape:getChildCount(): number",
            documentation: r"Returns the number of children the shape has.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"getRadius",
            kind: LuaItemKind::Function,
            detail: r"function Shape:getRadius(): number",
            documentation: r"Gets the radius of the shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function Shape:getType(): ShapeType",
            documentation: r"Gets a string representing the Shape.

This function can be useful for conditional debug drawing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"rayCast",
            kind: LuaItemKind::Function,
            detail: r"function Shape:rayCast(x1: number, y1: number, x2: number, y2: number, maxFraction: number, tx: number, ty: number, tr: number, childIndex?: number): number, number, number",
            documentation: r"Casts a ray against the shape and returns the surface normal vector and the line position where the ray hit. If the ray missed the shape, nil will be returned. The Shape can be transformed to get it into the desired position.

The ray starts on the first point of the input line and goes towards the second point of the line. The fourth argument is the maximum distance the ray is going to travel as a scale factor of the input line length.

The childIndex parameter is used to specify which child of a parent shape, such as a ChainShape, will be ray casted. For ChainShapes, the index of 1 is the first edge on the chain. Ray casting a parent shape will only test the child specified so if you want to test every shape of the parent, you must loop through all of its children.

The world position of the impact can be calculated by multiplying the line vector with the third return value and adding it to the line starting point.

hitx, hity = x1 + (x2 - x1) * fraction, y1 + (y2 - y1) * fraction",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"testPoint",
            kind: LuaItemKind::Function,
            detail: r"function Shape:testPoint(tx: number, ty: number, tr: number, x: number, y: number): boolean",
            documentation: r"This is particularly useful for mouse interaction with the shapes. By looping through all shapes and testing the mouse position with this function, we can find which shapes the mouse touches.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Shape:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Shape:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Shape",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Shape:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getBitDepth",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getBitDepth(): number",
            documentation: r"Returns the number of bits per sample.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getChannelCount",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getChannelCount(): number",
            documentation: r"Returns the number of channels in the SoundData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getDuration",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getDuration(): number",
            documentation: r"Gets the duration of the sound data.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getSample",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getSample(i: number): number",
            documentation: r"Gets the value of the sample-point at the specified position. For stereo SoundData objects, the data from the left and right channels are interleaved in that order.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getSampleCount",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getSampleCount(): number",
            documentation: r"Returns the number of samples per channel of the SoundData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getSampleRate",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getSampleRate(): number",
            documentation: r"Returns the sample rate of the SoundData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"setSample",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:setSample(i: number, sample: number)",
            documentation: r"Sets the value of the sample-point at the specified position. For stereo SoundData objects, the data from the left and right channels are interleaved in that order.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:clone(): Data",
            documentation: r"Creates a new copy of the Data object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getFFIPointer",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getFFIPointer(): cdata",
            documentation: r"Gets an FFI pointer to the Data.

This function should be preferred instead of Data:getPointer because the latter uses light userdata which can't store more all possible memory addresses on some new ARM64 architectures, when LuaJIT is used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getPointer",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getPointer(): light userdata",
            documentation: r"Gets a pointer to the Data. Can be used with libraries such as LuaJIT's FFI.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getSize",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getSize(): number",
            documentation: r"Gets the Data's size in bytes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"getString",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:getString(): string",
            documentation: r"Gets the full Data as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SoundData",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function SoundData:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function Source:clone(): Source",
            documentation: r"Creates an identical copy of the Source in the stopped state.

Static Sources will use significantly less memory and take much less time to be created if Source:clone is used to create them instead of love.audio.newSource, so this method should be preferred when making multiple Sources which play the same sound.

Cloned Sources inherit all the set-able state of the original Source, but they are initialized stopped.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getActiveEffects",
            kind: LuaItemKind::Function,
            detail: r"function Source:getActiveEffects(): table",
            documentation: r"Gets a list of the Source's active effect names.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getAirAbsorption",
            kind: LuaItemKind::Function,
            detail: r"function Source:getAirAbsorption(): number",
            documentation: r"Gets the amount of air absorption applied to the Source.

By default the value is set to 0 which means that air absorption effects are disabled. A value of 1 will apply high frequency attenuation to the Source at a rate of 0.05 dB per meter.

Audio air absorption functionality is not supported on iOS.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getAttenuationDistances",
            kind: LuaItemKind::Function,
            detail: r"function Source:getAttenuationDistances(): number, number",
            documentation: r"Gets the reference and maximum attenuation distances of the Source. The values, combined with the current DistanceModel, affect how the Source's volume attenuates based on distance from the listener.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getChannelCount",
            kind: LuaItemKind::Function,
            detail: r"function Source:getChannelCount(): number",
            documentation: r"Gets the number of channels in the Source. Only 1-channel (mono) Sources can use directional and positional effects.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getCone",
            kind: LuaItemKind::Function,
            detail: r"function Source:getCone(): number, number, number",
            documentation: r"Gets the Source's directional volume cones. Together with Source:setDirection, the cone angles allow for the Source's volume to vary depending on its direction.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getDirection",
            kind: LuaItemKind::Function,
            detail: r"function Source:getDirection(): number, number, number",
            documentation: r"Gets the direction of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getDuration",
            kind: LuaItemKind::Function,
            detail: r"function Source:getDuration(unit?: TimeUnit): number",
            documentation: r"Gets the duration of the Source. For streaming Sources it may not always be sample-accurate, and may return -1 if the duration cannot be determined at all.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getEffect",
            kind: LuaItemKind::Function,
            detail: r"function Source:getEffect(name: string, filtersettings?: table): table",
            documentation: r"Gets the filter settings associated to a specific effect.

This function returns nil if the effect was applied with no filter settings associated to it.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getFilter",
            kind: LuaItemKind::Function,
            detail: r"function Source:getFilter(): table",
            documentation: r"Gets the filter settings currently applied to the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getFreeBufferCount",
            kind: LuaItemKind::Function,
            detail: r"function Source:getFreeBufferCount(): number",
            documentation: r"Gets the number of free buffer slots in a queueable Source. If the queueable Source is playing, this value will increase up to the amount the Source was created with. If the queueable Source is stopped, it will process all of its internal buffers first, in which case this function will always return the amount it was created with.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getPitch",
            kind: LuaItemKind::Function,
            detail: r"function Source:getPitch(): number",
            documentation: r"Gets the current pitch of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getPosition",
            kind: LuaItemKind::Function,
            detail: r"function Source:getPosition(): number, number, number",
            documentation: r"Gets the position of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getRolloff",
            kind: LuaItemKind::Function,
            detail: r"function Source:getRolloff(): number",
            documentation: r"Returns the rolloff factor of the source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function Source:getType(): SourceType",
            documentation: r"Gets the type of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getVelocity",
            kind: LuaItemKind::Function,
            detail: r"function Source:getVelocity(): number, number, number",
            documentation: r"Gets the velocity of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getVolume",
            kind: LuaItemKind::Function,
            detail: r"function Source:getVolume(): number",
            documentation: r"Gets the current volume of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"getVolumeLimits",
            kind: LuaItemKind::Function,
            detail: r"function Source:getVolumeLimits(): number, number",
            documentation: r"Returns the volume limits of the source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"isLooping",
            kind: LuaItemKind::Function,
            detail: r"function Source:isLooping(): boolean",
            documentation: r"Returns whether the Source will loop.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"isPlaying",
            kind: LuaItemKind::Function,
            detail: r"function Source:isPlaying(): boolean",
            documentation: r"Returns whether the Source is playing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"isRelative",
            kind: LuaItemKind::Function,
            detail: r"function Source:isRelative(): boolean",
            documentation: r"Gets whether the Source's position, velocity, direction, and cone angles are relative to the listener.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"pause",
            kind: LuaItemKind::Function,
            detail: r"function Source:pause()",
            documentation: r"Pauses the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"play",
            kind: LuaItemKind::Function,
            detail: r"function Source:play(): boolean",
            documentation: r"Starts playing the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"queue",
            kind: LuaItemKind::Function,
            detail: r"function Source:queue(sounddata: SoundData): boolean",
            documentation: r"Queues SoundData for playback in a queueable Source.

This method requires the Source to be created via love.audio.newQueueableSource.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"seek",
            kind: LuaItemKind::Function,
            detail: r"function Source:seek(offset: number, unit?: TimeUnit)",
            documentation: r"Sets the currently playing position of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setAirAbsorption",
            kind: LuaItemKind::Function,
            detail: r"function Source:setAirAbsorption(amount: number)",
            documentation: r"Sets the amount of air absorption applied to the Source.

By default the value is set to 0 which means that air absorption effects are disabled. A value of 1 will apply high frequency attenuation to the Source at a rate of 0.05 dB per meter.

Air absorption can simulate sound transmission through foggy air, dry air, smoky atmosphere, etc. It can be used to simulate different atmospheric conditions within different locations in an area.

Audio air absorption functionality is not supported on iOS.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setAttenuationDistances",
            kind: LuaItemKind::Function,
            detail: r"function Source:setAttenuationDistances(ref: number, max: number)",
            documentation: r"Sets the reference and maximum attenuation distances of the Source. The parameters, combined with the current DistanceModel, affect how the Source's volume attenuates based on distance.

Distance attenuation is only applicable to Sources based on mono (rather than stereo) audio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setCone",
            kind: LuaItemKind::Function,
            detail: r"function Source:setCone(innerAngle: number, outerAngle: number, outerVolume?: number)",
            documentation: r"Sets the Source's directional volume cones. Together with Source:setDirection, the cone angles allow for the Source's volume to vary depending on its direction.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setDirection",
            kind: LuaItemKind::Function,
            detail: r"function Source:setDirection(x: number, y: number, z: number)",
            documentation: r"Sets the direction vector of the Source. A zero vector makes the source non-directional.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setEffect",
            kind: LuaItemKind::Function,
            detail: r"function Source:setEffect(name: string, enable?: boolean): boolean",
            documentation: r"Applies an audio effect to the Source.

The effect must have been previously defined using love.audio.setEffect.

Applies the given previously defined effect to this Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setFilter",
            kind: LuaItemKind::Function,
            detail: r"function Source:setFilter(settings: table): boolean",
            documentation: r"Sets a low-pass, high-pass, or band-pass filter to apply when playing the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setLooping",
            kind: LuaItemKind::Function,
            detail: r"function Source:setLooping(loop: boolean)",
            documentation: r"Sets whether the Source should loop.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setPitch",
            kind: LuaItemKind::Function,
            detail: r"function Source:setPitch(pitch: number)",
            documentation: r"Sets the pitch of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setPosition",
            kind: LuaItemKind::Function,
            detail: r"function Source:setPosition(x: number, y: number, z: number)",
            documentation: r"Sets the position of the Source. Please note that this only works for mono (i.e. non-stereo) sound files!",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setRelative",
            kind: LuaItemKind::Function,
            detail: r"function Source:setRelative(enable?: boolean)",
            documentation: r"Sets whether the Source's position, velocity, direction, and cone angles are relative to the listener, or absolute.

By default, all sources are absolute and therefore relative to the origin of love's coordinate system 0, 0. Only absolute sources are affected by the position of the listener. Please note that positional audio only works for mono (i.e. non-stereo) sources. ",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setRolloff",
            kind: LuaItemKind::Function,
            detail: r"function Source:setRolloff(rolloff: number)",
            documentation: r"Sets the rolloff factor which affects the strength of the used distance attenuation.

Extended information and detailed formulas can be found in the chapter '3.4. Attenuation By Distance' of OpenAL 1.1 specification.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setVelocity",
            kind: LuaItemKind::Function,
            detail: r"function Source:setVelocity(x: number, y: number, z: number)",
            documentation: r"Sets the velocity of the Source.

This does '''not''' change the position of the Source, but lets the application know how it has to calculate the doppler effect.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setVolume",
            kind: LuaItemKind::Function,
            detail: r"function Source:setVolume(volume: number)",
            documentation: r"Sets the current volume of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"setVolumeLimits",
            kind: LuaItemKind::Function,
            detail: r"function Source:setVolumeLimits(min: number, max: number)",
            documentation: r"Sets the volume limits of the source. The limits have to be numbers from 0 to 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"stop",
            kind: LuaItemKind::Function,
            detail: r"function Source:stop()",
            documentation: r"Stops a Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"tell",
            kind: LuaItemKind::Function,
            detail: r"function Source:tell(unit?: TimeUnit): number",
            documentation: r"Gets the currently playing position of the Source.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Source:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Source:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Source",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Source:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"add",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:add(x: number, y: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number): number",
            documentation: r"Adds a sprite to the batch. Sprites are drawn in the order they are added.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"addLayer",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:addLayer(layerindex: number, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number): number",
            documentation: r"Adds a sprite to a batch created with an Array Texture.

Adds a layer of the SpriteBatch's Array Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"attachAttribute",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:attachAttribute(name: string, mesh: Mesh)",
            documentation: r"Attaches a per-vertex attribute from a Mesh onto this SpriteBatch, for use when drawing. This can be combined with a Shader to augment a SpriteBatch with per-vertex or additional per-sprite information instead of just having per-sprite colors.

Each sprite in a SpriteBatch has 4 vertices in the following order: top-left, bottom-left, top-right, bottom-right. The index returned by SpriteBatch:add (and used by SpriteBatch:set) can used to determine the first vertex of a specific sprite with the formula 1 + 4 * ( id - 1 ).

If a created with a custom vertex format, it will have 3 vertex attributes named VertexPosition, VertexTexCoord, and VertexColor. If vertex attributes with those names are attached to the SpriteBatch, it will override the SpriteBatch's sprite positions, texture coordinates, and sprite colors, respectively.

Custom named attributes can be accessed in a vertex shader by declaring them as attribute vec4 MyCustomAttributeName; at the top-level of the vertex shader code. The name must match what was specified in the Mesh's vertex format and in the name argument of SpriteBatch:attachAttribute.

A Mesh must have at least 4 * SpriteBatch:getBufferSize vertices in order to be attachable to a SpriteBatch.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"clear",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:clear()",
            documentation: r"Removes all sprites from the buffer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"flush",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:flush()",
            documentation: r"Immediately sends all new and modified sprite data in the batch to the graphics card.

Normally it isn't necessary to call this method as love.graphics.draw(spritebatch, ...) will do it automatically if needed, but explicitly using SpriteBatch:flush gives more control over when the work happens.

If this method is used, it generally shouldn't be called more than once (at most) between love.graphics.draw(spritebatch, ...) calls.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"getBufferSize",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:getBufferSize(): number",
            documentation: r"Gets the maximum number of sprites the SpriteBatch can hold.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"getColor",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:getColor(): number, number, number, number",
            documentation: r"Gets the color that will be used for the next add and set operations.

If no color has been set with SpriteBatch:setColor or the current SpriteBatch color has been cleared, this method will return nil.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"getCount",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:getCount(): number",
            documentation: r"Gets the number of sprites currently in the SpriteBatch.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"getTexture",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:getTexture(): Texture",
            documentation: r"Gets the texture (Image or Canvas) used by the SpriteBatch.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"set",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:set(spriteindex: number, x: number, y: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)",
            documentation: r"Changes a sprite in the batch. This requires the sprite index returned by SpriteBatch:add or SpriteBatch:addLayer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"setColor",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:setColor(r: number, g: number, b: number, a?: number)",
            documentation: r"Sets the color that will be used for the next add and set operations. Calling the function without arguments will disable all per-sprite colors for the SpriteBatch.

In versions prior to 11.0, color component values were within the range of 0 to 255 instead of 0 to 1.

In version 0.9.2 and older, the global color set with love.graphics.setColor will not work on the SpriteBatch if any of the sprites has its own color.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"setDrawRange",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:setDrawRange(start: number, count: number)",
            documentation: r"Restricts the drawn sprites in the SpriteBatch to a subset of the total.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"setLayer",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:setLayer(spriteindex: number, layerindex: number, x?: number, y?: number, r?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number)",
            documentation: r"Changes a sprite previously added with add or addLayer, in a batch created with an Array Texture.

Changes the sprite in the SpriteBatch.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"setTexture",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:setTexture(texture: Texture)",
            documentation: r"Sets the texture (Image or Canvas) used for the sprites in the batch, when drawing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatch",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function SpriteBatch:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"add",
            kind: LuaItemKind::Function,
            detail: r"function Text:add(textstring: string, x?: number, y?: number, angle?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number): number",
            documentation: r"Adds additional colored text to the Text object at the specified position.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"addf",
            kind: LuaItemKind::Function,
            detail: r"function Text:addf(textstring: string, wraplimit: number, align: AlignMode, x: number, y: number, angle?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number): number",
            documentation: r"Adds additional formatted / colored text to the Text object at the specified position.

The word wrap limit is applied before any scaling, rotation, and other coordinate transformations. Therefore the amount of text per line stays constant given the same wrap limit, even if the scale arguments change.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"clear",
            kind: LuaItemKind::Function,
            detail: r"function Text:clear()",
            documentation: r"Clears the contents of the Text object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"getDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Text:getDimensions(): number, number",
            documentation: r"Gets the width and height of the text in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"getFont",
            kind: LuaItemKind::Function,
            detail: r"function Text:getFont(): Font",
            documentation: r"Gets the Font used with the Text object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function Text:getHeight(): number",
            documentation: r"Gets the height of the text in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function Text:getWidth(): number",
            documentation: r"Gets the width of the text in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"set",
            kind: LuaItemKind::Function,
            detail: r"function Text:set(textstring: string)",
            documentation: r"Replaces the contents of the Text object with a new unformatted string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"setFont",
            kind: LuaItemKind::Function,
            detail: r"function Text:setFont(font: Font)",
            documentation: r"Replaces the Font used with the text.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"setf",
            kind: LuaItemKind::Function,
            detail: r"function Text:setf(textstring: string, wraplimit: number, align: AlignMode)",
            documentation: r"Replaces the contents of the Text object with a new formatted string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Text:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Text:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Text",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Text:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getDPIScale",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getDPIScale(): number",
            documentation: r"Gets the DPI scale factor of the Texture.

The DPI scale factor represents relative pixel density. A DPI scale factor of 2 means the texture has twice the pixel density in each dimension (4 times as many pixels in the same area) compared to a texture with a DPI scale factor of 1.

For example, a texture with pixel dimensions of 100x100 with a DPI scale factor of 2 will be drawn as if it was 50x50. This is useful with high-dpi /  retina displays to easily allow swapping out higher or lower pixel density Images and Canvases without needing any extra manual scaling logic.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getDepth",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getDepth(): number",
            documentation: r"Gets the depth of a Volume Texture. Returns 1 for 2D, Cubemap, and Array textures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getDepthSampleMode",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getDepthSampleMode(): CompareMode",
            documentation: r"Gets the comparison mode used when sampling from a depth texture in a shader.

Depth texture comparison modes are advanced low-level functionality typically used with shadow mapping in 3D.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getDimensions(): number, number",
            documentation: r"Gets the width and height of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getFilter",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getFilter(): FilterMode, FilterMode, number",
            documentation: r"Gets the filter mode of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getFormat",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getFormat(): PixelFormat",
            documentation: r"Gets the pixel format of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getHeight(): number",
            documentation: r"Gets the height of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getLayerCount",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getLayerCount(): number",
            documentation: r"Gets the number of layers / slices in an Array Texture. Returns 1 for 2D, Cubemap, and Volume textures.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getMipmapCount",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getMipmapCount(): number",
            documentation: r"Gets the number of mipmaps contained in the Texture. If the texture was not created with mipmaps, it will return 1.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getMipmapFilter",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getMipmapFilter(): FilterMode, number",
            documentation: r"Gets the mipmap filter mode for a Texture. Prior to 11.0 this method only worked on Images.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getPixelDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getPixelDimensions(): number, number",
            documentation: r"Gets the width and height in pixels of the Texture.

Texture:getDimensions gets the dimensions of the texture in units scaled by the texture's DPI scale factor, rather than pixels. Use getDimensions for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelDimensions only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getPixelHeight",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getPixelHeight(): number",
            documentation: r"Gets the height in pixels of the Texture.

DPI scale factor, rather than pixels. Use getHeight for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelHeight only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getPixelWidth",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getPixelWidth(): number",
            documentation: r"Gets the width in pixels of the Texture.

DPI scale factor, rather than pixels. Use getWidth for calculations related to drawing the texture (calculating an origin offset, for example), and getPixelWidth only when dealing specifically with pixels, for example when using Canvas:newImageData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getTextureType",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getTextureType(): TextureType",
            documentation: r"Gets the type of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getWidth(): number",
            documentation: r"Gets the width of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"getWrap",
            kind: LuaItemKind::Function,
            detail: r"function Texture:getWrap(): WrapMode, WrapMode, WrapMode",
            documentation: r"Gets the wrapping properties of a Texture.

This function returns the currently set horizontal and vertical wrapping modes for the texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"isReadable",
            kind: LuaItemKind::Function,
            detail: r"function Texture:isReadable(): boolean",
            documentation: r"Gets whether the Texture can be drawn and sent to a Shader.

Canvases created with stencil and/or depth PixelFormats are not readable by default, unless readable=true is specified in the settings table passed into love.graphics.newCanvas.

Non-readable Canvases can still be rendered to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"setDepthSampleMode",
            kind: LuaItemKind::Function,
            detail: r"function Texture:setDepthSampleMode(compare: CompareMode)",
            documentation: r"Sets the comparison mode used when sampling from a depth texture in a shader. Depth texture comparison modes are advanced low-level functionality typically used with shadow mapping in 3D.

When using a depth texture with a comparison mode set in a shader, it must be declared as a sampler2DShadow and used in a GLSL 3 Shader. The result of accessing the texture in the shader will return a float between 0 and 1, proportional to the number of samples (up to 4 samples will be used if bilinear filtering is enabled) that passed the test set by the comparison operation.

Depth texture comparison can only be used with readable depth-formatted Canvases.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"setFilter",
            kind: LuaItemKind::Function,
            detail: r"function Texture:setFilter(min: FilterMode, mag?: FilterMode, anisotropy?: number)",
            documentation: r"Sets the filter mode of the Texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"setMipmapFilter",
            kind: LuaItemKind::Function,
            detail: r"function Texture:setMipmapFilter(filtermode: FilterMode, sharpness?: number)",
            documentation: r"Sets the mipmap filter mode for a Texture. Prior to 11.0 this method only worked on Images.

Mipmapping is useful when drawing a texture at a reduced scale. It can improve performance and reduce aliasing issues.

In created with the mipmaps flag enabled for the mipmap filter to have any effect. In versions prior to 0.10.0 it's best to call this method directly after creating the image with love.graphics.newImage, to avoid bugs in certain graphics drivers.

Due to hardware restrictions and driver bugs, in versions prior to 0.10.0 images that weren't loaded from a CompressedData must have power-of-two dimensions (64x64, 512x256, etc.) to use mipmaps.

On mobile devices (Android and iOS), the sharpness parameter is not supported and will do nothing. You can use a custom compressed and its CompressedData has mipmap data included, it will use that.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"setWrap",
            kind: LuaItemKind::Function,
            detail: r"function Texture:setWrap(horiz: WrapMode, vert?: WrapMode, depth?: WrapMode)",
            documentation: r"Sets the wrapping properties of a Texture.

This function sets the way a Texture is repeated when it is drawn with a Quad that is larger than the texture's extent, or when a custom Shader is used which uses texture coordinates outside of [0, 1]. A texture may be clamped or set to repeat in both horizontal and vertical directions.

Clamped textures appear only once (with the edges of the texture stretching to fill the extent of the Quad), whereas repeated ones repeat as many times as there is room in the Quad.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Texture:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Texture:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Texture",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Texture:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Thread",
        item: LuaItem {
            label: r"getError",
            kind: LuaItemKind::Function,
            detail: r"function Thread:getError(): string",
            documentation: r"Retrieves the error string from the thread if it produced an error.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Thread",
        item: LuaItem {
            label: r"isRunning",
            kind: LuaItemKind::Function,
            detail: r"function Thread:isRunning(): boolean",
            documentation: r"Returns whether the thread is currently running.

Threads which are not running can be (re)started with Thread:start.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Thread",
        item: LuaItem {
            label: r"start",
            kind: LuaItemKind::Function,
            detail: r"function Thread:start()",
            documentation: r"Starts the thread.

Beginning with version 0.9.0, threads can be restarted after they have completed their execution.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Thread",
        item: LuaItem {
            label: r"wait",
            kind: LuaItemKind::Function,
            detail: r"function Thread:wait()",
            documentation: r"Wait for a thread to finish.

This call will block until the thread finishes.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Thread",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Thread:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Thread",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Thread:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Thread",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Thread:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"apply",
            kind: LuaItemKind::Function,
            detail: r"function Transform:apply(other: Transform): Transform",
            documentation: r"Applies the given other Transform object to this one.

This effectively multiplies this Transform's internal transformation matrix with the other Transform's (i.e. self * other), and stores the result in this object.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"clone",
            kind: LuaItemKind::Function,
            detail: r"function Transform:clone(): Transform",
            documentation: r"Creates a new copy of this Transform.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"getMatrix",
            kind: LuaItemKind::Function,
            detail: r"function Transform:getMatrix(): number, number, number, number, number, number, number, number, number, number, number, number, number, number, number, number",
            documentation: r"Gets the internal 4x4 transformation matrix stored by this Transform. The matrix is returned in row-major order.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"inverse",
            kind: LuaItemKind::Function,
            detail: r"function Transform:inverse(): Transform",
            documentation: r"Creates a new Transform containing the inverse of this Transform.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"inverseTransformPoint",
            kind: LuaItemKind::Function,
            detail: r"function Transform:inverseTransformPoint(localX: number, localY: number): number, number",
            documentation: r"Applies the reverse of the Transform object's transformation to the given 2D position.

This effectively converts the given position from the local coordinate space of the Transform into global coordinates.

One use of this method can be to convert a screen-space mouse position into global world coordinates, if the given Transform has transformations applied that are used for a camera system in-game.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"isAffine2DTransform",
            kind: LuaItemKind::Function,
            detail: r"function Transform:isAffine2DTransform(): boolean",
            documentation: r"Checks whether the Transform is an affine transformation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"reset",
            kind: LuaItemKind::Function,
            detail: r"function Transform:reset(): Transform",
            documentation: r"Resets the Transform to an identity state. All previously applied transformations are erased.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"rotate",
            kind: LuaItemKind::Function,
            detail: r"function Transform:rotate(angle: number): Transform",
            documentation: r"Applies a rotation to the Transform's coordinate system. This method does not reset any previously applied transformations.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"scale",
            kind: LuaItemKind::Function,
            detail: r"function Transform:scale(sx: number, sy?: number): Transform",
            documentation: r"Scales the Transform's coordinate system. This method does not reset any previously applied transformations.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"setMatrix",
            kind: LuaItemKind::Function,
            detail: r"function Transform:setMatrix(e1_1: number, e1_2: number, e1_3: number, e1_4: number, e2_1: number, e2_2: number, e2_3: number, e2_4: number, e3_1: number, e3_2: number, e3_3: number, e3_4: number, e4_1: number, e4_2: number, e4_3: number, e4_4: number): Transform",
            documentation: r"Directly sets the Transform's internal 4x4 transformation matrix.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"setTransformation",
            kind: LuaItemKind::Function,
            detail: r"function Transform:setTransformation(x: number, y: number, angle?: number, sx?: number, sy?: number, ox?: number, oy?: number, kx?: number, ky?: number): Transform",
            documentation: r"Resets the Transform to the specified transformation parameters.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"shear",
            kind: LuaItemKind::Function,
            detail: r"function Transform:shear(kx: number, ky: number): Transform",
            documentation: r"Applies a shear factor (skew) to the Transform's coordinate system. This method does not reset any previously applied transformations.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"transformPoint",
            kind: LuaItemKind::Function,
            detail: r"function Transform:transformPoint(globalX: number, globalY: number): number, number",
            documentation: r"Applies the Transform object's transformation to the given 2D position.

This effectively converts the given position from global coordinates into the local coordinate space of the Transform.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"translate",
            kind: LuaItemKind::Function,
            detail: r"function Transform:translate(dx: number, dy: number): Transform",
            documentation: r"Applies a translation to the Transform's coordinate system. This method does not reset any previously applied transformations.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Transform:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Transform:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Transform",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Transform:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"getDimensions",
            kind: LuaItemKind::Function,
            detail: r"function Video:getDimensions(): number, number",
            documentation: r"Gets the width and height of the Video in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"getFilter",
            kind: LuaItemKind::Function,
            detail: r"function Video:getFilter(): FilterMode, FilterMode, number",
            documentation: r"Gets the scaling filters used when drawing the Video.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"getHeight",
            kind: LuaItemKind::Function,
            detail: r"function Video:getHeight(): number",
            documentation: r"Gets the height of the Video in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"getSource",
            kind: LuaItemKind::Function,
            detail: r"function Video:getSource(): Source",
            documentation: r"Gets the audio Source used for playing back the video's audio. May return nil if the video has no audio, or if Video:setSource is called with a nil argument.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"getStream",
            kind: LuaItemKind::Function,
            detail: r"function Video:getStream(): VideoStream",
            documentation: r"Gets the VideoStream object used for decoding and controlling the video.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"getWidth",
            kind: LuaItemKind::Function,
            detail: r"function Video:getWidth(): number",
            documentation: r"Gets the width of the Video in pixels.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"isPlaying",
            kind: LuaItemKind::Function,
            detail: r"function Video:isPlaying(): boolean",
            documentation: r"Gets whether the Video is currently playing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"pause",
            kind: LuaItemKind::Function,
            detail: r"function Video:pause()",
            documentation: r"Pauses the Video.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"play",
            kind: LuaItemKind::Function,
            detail: r"function Video:play()",
            documentation: r"Starts playing the Video. In order for the video to appear onscreen it must be drawn with love.graphics.draw.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"rewind",
            kind: LuaItemKind::Function,
            detail: r"function Video:rewind()",
            documentation: r"Rewinds the Video to the beginning.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"seek",
            kind: LuaItemKind::Function,
            detail: r"function Video:seek(offset: number)",
            documentation: r"Sets the current playback position of the Video.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"setFilter",
            kind: LuaItemKind::Function,
            detail: r"function Video:setFilter(min: FilterMode, mag: FilterMode, anisotropy?: number)",
            documentation: r"Sets the scaling filters used when drawing the Video.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"setSource",
            kind: LuaItemKind::Function,
            detail: r"function Video:setSource(source?: Source)",
            documentation: r"Sets the audio Source used for playing back the video's audio. The audio Source also controls playback speed and synchronization.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"tell",
            kind: LuaItemKind::Function,
            detail: r"function Video:tell(): number",
            documentation: r"Gets the current playback position of the Video.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function Video:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function Video:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Video",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function Video:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"getFilename",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:getFilename(): string",
            documentation: r"Gets the filename of the VideoStream.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"isPlaying",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:isPlaying(): boolean",
            documentation: r"Gets whether the VideoStream is playing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"pause",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:pause()",
            documentation: r"Pauses the VideoStream.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"play",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:play()",
            documentation: r"Plays the VideoStream.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"rewind",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:rewind()",
            documentation: r"Rewinds the VideoStream. Synonym to VideoStream:seek(0).",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"seek",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:seek(offset: number)",
            documentation: r"Sets the current playback position of the VideoStream.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"tell",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:tell(): number",
            documentation: r"Gets the current playback position of the VideoStream.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VideoStream",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function VideoStream:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getDampingRatio",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getDampingRatio(): number",
            documentation: r"Returns the damping ratio of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getFrequency",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getFrequency(): number",
            documentation: r"Returns the frequency.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getReferenceAngle",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getReferenceAngle(): number",
            documentation: r"Gets the reference angle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"setDampingRatio",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:setDampingRatio(ratio: number)",
            documentation: r"Sets a new damping ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"setFrequency",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:setFrequency(freq: number)",
            documentation: r"Sets a new frequency.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WeldJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function WeldJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getAxis",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getAxis(): number, number",
            documentation: r"Gets the world-space axis vector of the Wheel Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getJointSpeed",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getJointSpeed(): number",
            documentation: r"Returns the current joint translation speed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getJointTranslation",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getJointTranslation(): number",
            documentation: r"Returns the current joint translation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getMaxMotorTorque",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getMaxMotorTorque(): number",
            documentation: r"Returns the maximum motor torque.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getMotorSpeed",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getMotorSpeed(): number",
            documentation: r"Returns the speed of the motor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getMotorTorque",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getMotorTorque(invdt: number): number",
            documentation: r"Returns the current torque on the motor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getSpringDampingRatio",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getSpringDampingRatio(): number",
            documentation: r"Returns the damping ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getSpringFrequency",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getSpringFrequency(): number",
            documentation: r"Returns the spring frequency.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"isMotorEnabled",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:isMotorEnabled(): boolean",
            documentation: r"Checks if the joint motor is running.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"setMaxMotorTorque",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:setMaxMotorTorque(maxTorque: number)",
            documentation: r"Sets a new maximum motor torque.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"setMotorEnabled",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:setMotorEnabled(enable: boolean)",
            documentation: r"Starts and stops the joint motor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"setMotorSpeed",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:setMotorSpeed(speed: number)",
            documentation: r"Sets a new speed for the motor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"setSpringDampingRatio",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:setSpringDampingRatio(ratio: number)",
            documentation: r"Sets a new damping ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"setSpringFrequency",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:setSpringFrequency(freq: number)",
            documentation: r"Sets a new spring frequency.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:destroy()",
            documentation: r"Explicitly destroys the Joint. An error will occur if you attempt to use the object after calling this function.

In 0.7.2, when you don't have time to wait for garbage collection, this function 

may be used to free the object immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getAnchors",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getAnchors(): number, number, number, number",
            documentation: r"Get the anchor points of the joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getBodies(): Body, Body",
            documentation: r"Gets the bodies that the Joint is attached to.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getCollideConnected",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getCollideConnected(): boolean",
            documentation: r"Gets whether the connected Bodies collide.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getReactionForce",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getReactionForce(x: number): number, number",
            documentation: r"Returns the reaction force in newtons on the second body",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getReactionTorque",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getReactionTorque(invdt: number): number",
            documentation: r"Returns the reaction torque on the second body.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getType",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getType(): JointType",
            documentation: r"Gets a string representing the type.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"getUserData",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:getUserData(): any",
            documentation: r"Returns the Lua value associated with this Joint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:isDestroyed(): boolean",
            documentation: r"Gets whether the Joint is destroyed. Destroyed joints cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"setUserData",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:setUserData(value: any)",
            documentation: r"Associates a Lua value with the Joint.

To delete the reference, explicitly pass nil.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WheelJoint",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function WheelJoint:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"destroy",
            kind: LuaItemKind::Function,
            detail: r"function World:destroy()",
            documentation: r"Destroys the world, taking all bodies, joints, fixtures and their shapes with it. 

An error will occur if you attempt to use any of the destroyed objects after calling this function.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getBodies",
            kind: LuaItemKind::Function,
            detail: r"function World:getBodies(): table",
            documentation: r"Returns a table with all bodies.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getBodyCount",
            kind: LuaItemKind::Function,
            detail: r"function World:getBodyCount(): number",
            documentation: r"Returns the number of bodies in the world.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getCallbacks",
            kind: LuaItemKind::Function,
            detail: r"function World:getCallbacks(): function, function, function, function",
            documentation: r"Returns functions for the callbacks during the world update.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getContactCount",
            kind: LuaItemKind::Function,
            detail: r"function World:getContactCount(): number",
            documentation: r"Returns the number of contacts in the world.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getContactFilter",
            kind: LuaItemKind::Function,
            detail: r"function World:getContactFilter(): function",
            documentation: r"Returns the function for collision filtering.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getContacts",
            kind: LuaItemKind::Function,
            detail: r"function World:getContacts(): table",
            documentation: r"Returns a table with all Contacts.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getGravity",
            kind: LuaItemKind::Function,
            detail: r"function World:getGravity(): number, number",
            documentation: r"Get the gravity of the world.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getJointCount",
            kind: LuaItemKind::Function,
            detail: r"function World:getJointCount(): number",
            documentation: r"Returns the number of joints in the world.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"getJoints",
            kind: LuaItemKind::Function,
            detail: r"function World:getJoints(): table",
            documentation: r"Returns a table with all joints.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"isDestroyed",
            kind: LuaItemKind::Function,
            detail: r"function World:isDestroyed(): boolean",
            documentation: r"Gets whether the World is destroyed. Destroyed worlds cannot be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"isLocked",
            kind: LuaItemKind::Function,
            detail: r"function World:isLocked(): boolean",
            documentation: r"Returns if the world is updating its state.

This will return true inside the callbacks from World:setCallbacks.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"isSleepingAllowed",
            kind: LuaItemKind::Function,
            detail: r"function World:isSleepingAllowed(): boolean",
            documentation: r"Gets the sleep behaviour of the world.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"queryBoundingBox",
            kind: LuaItemKind::Function,
            detail: r"function World:queryBoundingBox(topLeftX: number, topLeftY: number, bottomRightX: number, bottomRightY: number, callback: function)",
            documentation: r"Calls a function for each fixture inside the specified area by searching for any overlapping bounding box (Fixture:getBoundingBox).",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"rayCast",
            kind: LuaItemKind::Function,
            detail: r"function World:rayCast(x1: number, y1: number, x2: number, y2: number, callback: function)",
            documentation: r"Casts a ray and calls a function for each fixtures it intersects. 

There is a bug in LÖVE 0.8.0 where the normal vector passed to the callback function gets scaled by love.physics.getMeter.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"setCallbacks",
            kind: LuaItemKind::Function,
            detail: r"function World:setCallbacks(beginContact: function, endContact: function, preSolve?: function, postSolve?: function)",
            documentation: r"Sets functions for the collision callbacks during the world update.

Four Lua functions can be given as arguments. The value nil removes a function.

When called, each function will be passed three arguments. The first two arguments are the colliding fixtures and the third argument is the Contact between them. The postSolve callback additionally gets the normal and tangent impulse for each contact point. See notes.

If you are interested to know when exactly each callback is called, consult a Box2d manual",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"setContactFilter",
            kind: LuaItemKind::Function,
            detail: r"function World:setContactFilter(filter: function)",
            documentation: r"Sets a function for collision filtering.

If the group and category filtering doesn't generate a collision decision, this function gets called with the two fixtures as arguments. The function should return a boolean value where true means the fixtures will collide and false means they will pass through each other.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"setGravity",
            kind: LuaItemKind::Function,
            detail: r"function World:setGravity(x: number, y: number)",
            documentation: r"Set the gravity of the world.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"setSleepingAllowed",
            kind: LuaItemKind::Function,
            detail: r"function World:setSleepingAllowed(allow: boolean)",
            documentation: r"Sets the sleep behaviour of the world.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"translateOrigin",
            kind: LuaItemKind::Function,
            detail: r"function World:translateOrigin(x: number, y: number)",
            documentation: r"Translates the World's origin. Useful in large worlds where floating point precision issues become noticeable at far distances from the origin.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"update",
            kind: LuaItemKind::Function,
            detail: r"function World:update(dt: number, velocityiterations?: number, positioniterations?: number)",
            documentation: r"Update the state of the world.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"release",
            kind: LuaItemKind::Function,
            detail: r"function World:release(): boolean",
            documentation: r"Destroys the object's Lua reference. The object will be completely deleted if it's not referenced by any other LÖVE object or thread.

This method can be used to immediately clean up resources without waiting for Lua's garbage collector.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"type",
            kind: LuaItemKind::Function,
            detail: r"function World:type(): string",
            documentation: r"Gets the type of the object as a string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"World",
        item: LuaItem {
            label: r"typeOf",
            kind: LuaItemKind::Function,
            detail: r"function World:typeOf(name: string): boolean",
            documentation: r"Checks whether an object is of a certain type. If the object has the type with the specified name in its hierarchy, this function will return true.",
        },
    },
];

const ENUM_ITEMS: &[LoveApiTypedItem] = &[
    LoveApiTypedItem {
        type_name: r"AlignMode",
        item: LuaItem {
            label: r"center",
            kind: LuaItemKind::Value,
            detail: r"AlignMode",
            documentation: r"Align text center.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AlignMode",
        item: LuaItem {
            label: r"left",
            kind: LuaItemKind::Value,
            detail: r"AlignMode",
            documentation: r"Align text left.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AlignMode",
        item: LuaItem {
            label: r"right",
            kind: LuaItemKind::Value,
            detail: r"AlignMode",
            documentation: r"Align text right.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AlignMode",
        item: LuaItem {
            label: r"justify",
            kind: LuaItemKind::Value,
            detail: r"AlignMode",
            documentation: r"Align text both left and right.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ArcType",
        item: LuaItem {
            label: r"pie",
            kind: LuaItemKind::Value,
            detail: r"ArcType",
            documentation: r"The arc is drawn like a slice of pie, with the arc circle connected to the center at its end-points.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ArcType",
        item: LuaItem {
            label: r"open",
            kind: LuaItemKind::Value,
            detail: r"ArcType",
            documentation: r#"The arc circle's two end-points are unconnected when the arc is drawn as a line. Behaves like the "closed" arc type when the arc is drawn in filled mode."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"ArcType",
        item: LuaItem {
            label: r"closed",
            kind: LuaItemKind::Value,
            detail: r"ArcType",
            documentation: r"The arc circle's two end-points are connected to each other.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AreaSpreadDistribution",
        item: LuaItem {
            label: r"uniform",
            kind: LuaItemKind::Value,
            detail: r"AreaSpreadDistribution",
            documentation: r"Uniform distribution.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AreaSpreadDistribution",
        item: LuaItem {
            label: r"normal",
            kind: LuaItemKind::Value,
            detail: r"AreaSpreadDistribution",
            documentation: r"Normal (gaussian) distribution.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AreaSpreadDistribution",
        item: LuaItem {
            label: r"ellipse",
            kind: LuaItemKind::Value,
            detail: r"AreaSpreadDistribution",
            documentation: r"Uniform distribution in an ellipse.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AreaSpreadDistribution",
        item: LuaItem {
            label: r"borderellipse",
            kind: LuaItemKind::Value,
            detail: r"AreaSpreadDistribution",
            documentation: r"Distribution in an ellipse with particles spawning at the edges of the ellipse.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AreaSpreadDistribution",
        item: LuaItem {
            label: r"borderrectangle",
            kind: LuaItemKind::Value,
            detail: r"AreaSpreadDistribution",
            documentation: r"Distribution in a rectangle with particles spawning at the edges of the rectangle.",
        },
    },
    LoveApiTypedItem {
        type_name: r"AreaSpreadDistribution",
        item: LuaItem {
            label: r"none",
            kind: LuaItemKind::Value,
            detail: r"AreaSpreadDistribution",
            documentation: r"No distribution - area spread is disabled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendAlphaMode",
        item: LuaItem {
            label: r"alphamultiply",
            kind: LuaItemKind::Value,
            detail: r"BlendAlphaMode",
            documentation: r"The RGB values of what's drawn are multiplied by the alpha values of those colors during blending. This is the default alpha mode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendAlphaMode",
        item: LuaItem {
            label: r"premultiplied",
            kind: LuaItemKind::Value,
            detail: r"BlendAlphaMode",
            documentation: r#"The RGB values of what's drawn are '''not''' multiplied by the alpha values of those colors during blending. For most blend modes to work correctly with this alpha mode, the colors of a drawn object need to have had their RGB values multiplied by their alpha values at some point previously ("premultiplied alpha")."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"alpha",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"Alpha blending (normal). The alpha of what's drawn determines its opacity.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"replace",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"The colors of what's drawn completely replace what was on the screen, with no additional blending. The BlendAlphaMode specified in love.graphics.setBlendMode still affects what happens.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"screen",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"'Screen' blending.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"add",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"The pixel colors of what's drawn are added to the pixel colors already on the screen. The alpha of the screen is not modified.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"subtract",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"The pixel colors of what's drawn are subtracted from the pixel colors already on the screen. The alpha of the screen is not modified.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"multiply",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r#"The pixel colors of what's drawn are multiplied with the pixel colors already on the screen (darkening them). The alpha of drawn objects is multiplied with the alpha of the screen rather than determining how much the colors on the screen are affected, even when the "alphamultiply" BlendAlphaMode is used."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"lighten",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r#"The pixel colors of what's drawn are compared to the existing pixel colors, and the larger of the two values for each color component is used. Only works when the "premultiplied" BlendAlphaMode is used in love.graphics.setBlendMode."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"darken",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r#"The pixel colors of what's drawn are compared to the existing pixel colors, and the smaller of the two values for each color component is used. Only works when the "premultiplied" BlendAlphaMode is used in love.graphics.setBlendMode."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"additive",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"Additive blend mode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"subtractive",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"Subtractive blend mode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"multiplicative",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"Multiply blend mode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BlendMode",
        item: LuaItem {
            label: r"premultiplied",
            kind: LuaItemKind::Value,
            detail: r"BlendMode",
            documentation: r"Premultiplied alpha blend mode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BodyType",
        item: LuaItem {
            label: r"static",
            kind: LuaItemKind::Value,
            detail: r"BodyType",
            documentation: r"Static bodies do not move.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BodyType",
        item: LuaItem {
            label: r"dynamic",
            kind: LuaItemKind::Value,
            detail: r"BodyType",
            documentation: r"Dynamic bodies collide with all bodies.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BodyType",
        item: LuaItem {
            label: r"kinematic",
            kind: LuaItemKind::Value,
            detail: r"BodyType",
            documentation: r"Kinematic bodies only collide with dynamic bodies.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BufferMode",
        item: LuaItem {
            label: r"none",
            kind: LuaItemKind::Value,
            detail: r"BufferMode",
            documentation: r"No buffering. The result of write and append operations appears immediately.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BufferMode",
        item: LuaItem {
            label: r"line",
            kind: LuaItemKind::Value,
            detail: r"BufferMode",
            documentation: r"Line buffering. Write and append operations are buffered until a newline is output or the buffer size limit is reached.",
        },
    },
    LoveApiTypedItem {
        type_name: r"BufferMode",
        item: LuaItem {
            label: r"full",
            kind: LuaItemKind::Value,
            detail: r"BufferMode",
            documentation: r"Full buffering. Write and append operations are always buffered until the buffer size limit is reached.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompareMode",
        item: LuaItem {
            label: r"equal",
            kind: LuaItemKind::Value,
            detail: r"CompareMode",
            documentation: r"* stencil tests: the stencil value of the pixel must be equal to the supplied value.
* depth tests: the depth value of the drawn object at that pixel must be equal to the existing depth value of that pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompareMode",
        item: LuaItem {
            label: r"notequal",
            kind: LuaItemKind::Value,
            detail: r"CompareMode",
            documentation: r"* stencil tests: the stencil value of the pixel must not be equal to the supplied value.
* depth tests: the depth value of the drawn object at that pixel must not be equal to the existing depth value of that pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompareMode",
        item: LuaItem {
            label: r"less",
            kind: LuaItemKind::Value,
            detail: r"CompareMode",
            documentation: r"* stencil tests: the stencil value of the pixel must be less than the supplied value.
* depth tests: the depth value of the drawn object at that pixel must be less than the existing depth value of that pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompareMode",
        item: LuaItem {
            label: r"lequal",
            kind: LuaItemKind::Value,
            detail: r"CompareMode",
            documentation: r"* stencil tests: the stencil value of the pixel must be less than or equal to the supplied value.
* depth tests: the depth value of the drawn object at that pixel must be less than or equal to the existing depth value of that pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompareMode",
        item: LuaItem {
            label: r"gequal",
            kind: LuaItemKind::Value,
            detail: r"CompareMode",
            documentation: r"* stencil tests: the stencil value of the pixel must be greater than or equal to the supplied value.
* depth tests: the depth value of the drawn object at that pixel must be greater than or equal to the existing depth value of that pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompareMode",
        item: LuaItem {
            label: r"greater",
            kind: LuaItemKind::Value,
            detail: r"CompareMode",
            documentation: r"* stencil tests: the stencil value of the pixel must be greater than the supplied value.
* depth tests: the depth value of the drawn object at that pixel must be greater than the existing depth value of that pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompareMode",
        item: LuaItem {
            label: r"never",
            kind: LuaItemKind::Value,
            detail: r"CompareMode",
            documentation: r"Objects will never be drawn.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompareMode",
        item: LuaItem {
            label: r"always",
            kind: LuaItemKind::Value,
            detail: r"CompareMode",
            documentation: r"Objects will always be drawn. Effectively disables the depth or stencil test.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedDataFormat",
        item: LuaItem {
            label: r"lz4",
            kind: LuaItemKind::Value,
            detail: r"CompressedDataFormat",
            documentation: r"The LZ4 compression format. Compresses and decompresses very quickly, but the compression ratio is not the best. LZ4-HC is used when compression level 9 is specified. Some benchmarks are available here.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedDataFormat",
        item: LuaItem {
            label: r"zlib",
            kind: LuaItemKind::Value,
            detail: r"CompressedDataFormat",
            documentation: r"The zlib format is DEFLATE-compressed data with a small bit of header data. Compresses relatively slowly and decompresses moderately quickly, and has a decent compression ratio.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedDataFormat",
        item: LuaItem {
            label: r"gzip",
            kind: LuaItemKind::Value,
            detail: r"CompressedDataFormat",
            documentation: r"The gzip format is DEFLATE-compressed data with a slightly larger header than zlib. Since it uses DEFLATE it has the same compression characteristics as the zlib format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedDataFormat",
        item: LuaItem {
            label: r"deflate",
            kind: LuaItemKind::Value,
            detail: r"CompressedDataFormat",
            documentation: r"Raw DEFLATE-compressed data (no header).",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"DXT1",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The DXT1 format. RGB data at 4 bits per pixel (compared to 32 bits for ImageData and regular Images.) Suitable for fully opaque images on desktop systems.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"DXT3",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The DXT3 format. RGBA data at 8 bits per pixel. Smooth variations in opacity do not mix well with this format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"DXT5",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The DXT5 format. RGBA data at 8 bits per pixel. Recommended for images with varying opacity on desktop systems.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"BC4",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The BC4 format (also known as 3Dc+ or ATI1.) Stores just the red channel, at 4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"BC4s",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The signed variant of the BC4 format. Same as above but pixel values in the texture are in the range of 1 instead of 1 in shaders.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"BC5",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The BC5 format (also known as 3Dc or ATI2.) Stores red and green channels at 8 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"BC5s",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The signed variant of the BC5 format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"BC6h",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The BC6H format. Stores half-precision floating-point RGB data in the range of 65504 at 8 bits per pixel. Suitable for HDR images on desktop systems.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"BC6hs",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The signed variant of the BC6H format. Stores RGB data in the range of +65504.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"BC7",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The BC7 format (also known as BPTC.) Stores RGB or RGBA data at 8 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ETC1",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The ETC1 format. RGB data at 4 bits per pixel. Suitable for fully opaque images on older Android devices.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ETC2rgb",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The RGB variant of the ETC2 format. RGB data at 4 bits per pixel. Suitable for fully opaque images on newer mobile devices.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ETC2rgba",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The RGBA variant of the ETC2 format. RGBA data at 8 bits per pixel. Recommended for images with varying opacity on newer mobile devices.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ETC2rgba1",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The RGBA variant of the ETC2 format where pixels are either fully transparent or fully opaque. RGBA data at 4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"EACr",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The single-channel variant of the EAC format. Stores just the red channel, at 4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"EACrs",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The signed single-channel variant of the EAC format. Same as above but pixel values in the texture are in the range of 1 instead of 1 in shaders.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"EACrg",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The two-channel variant of the EAC format. Stores red and green channels at 8 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"EACrgs",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The signed two-channel variant of the EAC format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"PVR1rgb2",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 2 bit per pixel RGB variant of the PVRTC1 format. Stores RGB data at 2 bits per pixel. Textures compressed with PVRTC1 formats must be square and power-of-two sized.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"PVR1rgb4",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 4 bit per pixel RGB variant of the PVRTC1 format. Stores RGB data at 4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"PVR1rgba2",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 2 bit per pixel RGBA variant of the PVRTC1 format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"PVR1rgba4",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 4 bit per pixel RGBA variant of the PVRTC1 format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC4x4",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 4x4 pixels per block variant of the ASTC format. RGBA data at 8 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC5x4",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 5x4 pixels per block variant of the ASTC format. RGBA data at 6.4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC5x5",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 5x5 pixels per block variant of the ASTC format. RGBA data at 5.12 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC6x5",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 6x5 pixels per block variant of the ASTC format. RGBA data at 4.27 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC6x6",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 6x6 pixels per block variant of the ASTC format. RGBA data at 3.56 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC8x5",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 8x5 pixels per block variant of the ASTC format. RGBA data at 3.2 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC8x6",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 8x6 pixels per block variant of the ASTC format. RGBA data at 2.67 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC8x8",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 8x8 pixels per block variant of the ASTC format. RGBA data at 2 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC10x5",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 10x5 pixels per block variant of the ASTC format. RGBA data at 2.56 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC10x6",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 10x6 pixels per block variant of the ASTC format. RGBA data at 2.13 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC10x8",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 10x8 pixels per block variant of the ASTC format. RGBA data at 1.6 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC10x10",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 10x10 pixels per block variant of the ASTC format. RGBA data at 1.28 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC12x10",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 12x10 pixels per block variant of the ASTC format. RGBA data at 1.07 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CompressedImageFormat",
        item: LuaItem {
            label: r"ASTC12x12",
            kind: LuaItemKind::Value,
            detail: r"CompressedImageFormat",
            documentation: r"The 12x12 pixels per block variant of the ASTC format. RGBA data at 0.89 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ContainerType",
        item: LuaItem {
            label: r"data",
            kind: LuaItemKind::Value,
            detail: r"ContainerType",
            documentation: r"Return type is ByteData.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ContainerType",
        item: LuaItem {
            label: r"string",
            kind: LuaItemKind::Value,
            detail: r"ContainerType",
            documentation: r"Return type is string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CullMode",
        item: LuaItem {
            label: r"back",
            kind: LuaItemKind::Value,
            detail: r"CullMode",
            documentation: r"Back-facing triangles in Meshes are culled (not rendered). The vertex order of a triangle determines whether it is back- or front-facing.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CullMode",
        item: LuaItem {
            label: r"front",
            kind: LuaItemKind::Value,
            detail: r"CullMode",
            documentation: r"Front-facing triangles in Meshes are culled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CullMode",
        item: LuaItem {
            label: r"none",
            kind: LuaItemKind::Value,
            detail: r"CullMode",
            documentation: r"Both back- and front-facing triangles in Meshes are rendered.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"image",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"The cursor is using a custom image.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"arrow",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"An arrow pointer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"ibeam",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"An I-beam, normally used when mousing over editable or selectable text.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"wait",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Wait graphic.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"waitarrow",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Small wait cursor with an arrow pointer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"crosshair",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Crosshair symbol.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"sizenwse",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Double arrow pointing to the top-left and bottom-right.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"sizenesw",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Double arrow pointing to the top-right and bottom-left.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"sizewe",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Double arrow pointing left and right.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"sizens",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Double arrow pointing up and down.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"sizeall",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Four-pointed arrow pointing up, down, left, and right.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"no",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Slashed circle or crossbones.",
        },
    },
    LoveApiTypedItem {
        type_name: r"CursorType",
        item: LuaItem {
            label: r"hand",
            kind: LuaItemKind::Value,
            detail: r"CursorType",
            documentation: r"Hand symbol.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DisplayOrientation",
        item: LuaItem {
            label: r"unknown",
            kind: LuaItemKind::Value,
            detail: r"DisplayOrientation",
            documentation: r"Orientation cannot be determined.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DisplayOrientation",
        item: LuaItem {
            label: r"landscape",
            kind: LuaItemKind::Value,
            detail: r"DisplayOrientation",
            documentation: r"Landscape orientation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DisplayOrientation",
        item: LuaItem {
            label: r"landscapeflipped",
            kind: LuaItemKind::Value,
            detail: r"DisplayOrientation",
            documentation: r"Landscape orientation (flipped).",
        },
    },
    LoveApiTypedItem {
        type_name: r"DisplayOrientation",
        item: LuaItem {
            label: r"portrait",
            kind: LuaItemKind::Value,
            detail: r"DisplayOrientation",
            documentation: r"Portrait orientation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DisplayOrientation",
        item: LuaItem {
            label: r"portraitflipped",
            kind: LuaItemKind::Value,
            detail: r"DisplayOrientation",
            documentation: r"Portrait orientation (flipped).",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceModel",
        item: LuaItem {
            label: r"none",
            kind: LuaItemKind::Value,
            detail: r"DistanceModel",
            documentation: r"Sources do not get attenuated.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceModel",
        item: LuaItem {
            label: r"inverse",
            kind: LuaItemKind::Value,
            detail: r"DistanceModel",
            documentation: r"Inverse distance attenuation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceModel",
        item: LuaItem {
            label: r"inverseclamped",
            kind: LuaItemKind::Value,
            detail: r"DistanceModel",
            documentation: r"Inverse distance attenuation. Gain is clamped. In version 0.9.2 and older this is named '''inverse clamped'''.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceModel",
        item: LuaItem {
            label: r"linear",
            kind: LuaItemKind::Value,
            detail: r"DistanceModel",
            documentation: r"Linear attenuation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceModel",
        item: LuaItem {
            label: r"linearclamped",
            kind: LuaItemKind::Value,
            detail: r"DistanceModel",
            documentation: r"Linear attenuation. Gain is clamped. In version 0.9.2 and older this is named '''linear clamped'''.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceModel",
        item: LuaItem {
            label: r"exponent",
            kind: LuaItemKind::Value,
            detail: r"DistanceModel",
            documentation: r"Exponential attenuation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DistanceModel",
        item: LuaItem {
            label: r"exponentclamped",
            kind: LuaItemKind::Value,
            detail: r"DistanceModel",
            documentation: r"Exponential attenuation. Gain is clamped. In version 0.9.2 and older this is named '''exponent clamped'''.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DrawMode",
        item: LuaItem {
            label: r"fill",
            kind: LuaItemKind::Value,
            detail: r"DrawMode",
            documentation: r"Draw filled shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"DrawMode",
        item: LuaItem {
            label: r"line",
            kind: LuaItemKind::Value,
            detail: r"DrawMode",
            documentation: r"Draw outlined shape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectType",
        item: LuaItem {
            label: r"chorus",
            kind: LuaItemKind::Value,
            detail: r"EffectType",
            documentation: r#"Plays multiple copies of the sound with slight pitch and time variation. Used to make sounds sound "fuller" or "thicker"."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectType",
        item: LuaItem {
            label: r"compressor",
            kind: LuaItemKind::Value,
            detail: r"EffectType",
            documentation: r"Decreases the dynamic range of the sound, making the loud and quiet parts closer in volume, producing a more uniform amplitude throughout time.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectType",
        item: LuaItem {
            label: r"distortion",
            kind: LuaItemKind::Value,
            detail: r"EffectType",
            documentation: r"Alters the sound by amplifying it until it clips, shearing off parts of the signal, leading to a compressed and distorted sound.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectType",
        item: LuaItem {
            label: r"echo",
            kind: LuaItemKind::Value,
            detail: r"EffectType",
            documentation: r"Decaying feedback based effect, on the order of seconds. Also known as delay; causes the sound to repeat at regular intervals at a decreasing volume.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectType",
        item: LuaItem {
            label: r"equalizer",
            kind: LuaItemKind::Value,
            detail: r"EffectType",
            documentation: r"Adjust the frequency components of the sound using a 4-band (low-shelf, two band-pass and a high-shelf) equalizer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectType",
        item: LuaItem {
            label: r"flanger",
            kind: LuaItemKind::Value,
            detail: r"EffectType",
            documentation: r"Plays two copies of the sound; while varying the phase, or equivalently delaying one of them, by amounts on the order of milliseconds, resulting in phasing sounds.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectType",
        item: LuaItem {
            label: r"reverb",
            kind: LuaItemKind::Value,
            detail: r"EffectType",
            documentation: r"Decaying feedback based effect, on the order of milliseconds. Used to simulate the reflection off of the surroundings.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectType",
        item: LuaItem {
            label: r"ringmodulator",
            kind: LuaItemKind::Value,
            detail: r"EffectType",
            documentation: r"An implementation of amplitude modulation; multiplies the source signal with a simple waveform, to produce either volume changes, or inharmonic overtones.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectWaveform",
        item: LuaItem {
            label: r"sawtooth",
            kind: LuaItemKind::Value,
            detail: r"EffectWaveform",
            documentation: r"A sawtooth wave, also known as a ramp wave. Named for its linear rise, and (near-)instantaneous fall along time.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectWaveform",
        item: LuaItem {
            label: r"sine",
            kind: LuaItemKind::Value,
            detail: r"EffectWaveform",
            documentation: r"A sine wave. Follows a trigonometric sine function.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectWaveform",
        item: LuaItem {
            label: r"square",
            kind: LuaItemKind::Value,
            detail: r"EffectWaveform",
            documentation: r"A square wave. Switches between high and low states (near-)instantaneously.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EffectWaveform",
        item: LuaItem {
            label: r"triangle",
            kind: LuaItemKind::Value,
            detail: r"EffectWaveform",
            documentation: r"A triangle wave. Follows a linear rise and fall that repeats periodically.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EncodeFormat",
        item: LuaItem {
            label: r"base64",
            kind: LuaItemKind::Value,
            detail: r"EncodeFormat",
            documentation: r"Encode/decode data as base64 binary-to-text encoding.",
        },
    },
    LoveApiTypedItem {
        type_name: r"EncodeFormat",
        item: LuaItem {
            label: r"hex",
            kind: LuaItemKind::Value,
            detail: r"EncodeFormat",
            documentation: r"Encode/decode data as hexadecimal string.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"focus",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Window focus gained or lost",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"joystickpressed",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick pressed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"joystickreleased",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick released",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"keypressed",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Key pressed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"keyreleased",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Key released",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"mousepressed",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Mouse pressed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"mousereleased",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Mouse released",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"quit",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Quit",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"resize",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Window size changed by the user",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"visible",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Window is minimized or un-minimized by the user",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"mousefocus",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Window mouse focus gained or lost",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"threaderror",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"A Lua error has occurred in a thread",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"joystickadded",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick connected",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"joystickremoved",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick disconnected",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"joystickaxis",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick axis motion",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"joystickhat",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick hat pressed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"gamepadpressed",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick's virtual gamepad button pressed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"gamepadreleased",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick's virtual gamepad button released",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"gamepadaxis",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick's virtual gamepad axis moved",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"textinput",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"User entered text",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"mousemoved",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Mouse position changed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"lowmemory",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Running out of memory on mobile devices system",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"textedited",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Candidate text for an IME changed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"wheelmoved",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Mouse wheel moved",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"touchpressed",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Touch screen touched",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"touchreleased",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Touch screen stop touching",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"touchmoved",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Touch press moved inside touch screen",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"directorydropped",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Directory is dragged and dropped onto the window",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"filedropped",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"File is dragged and dropped onto the window.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"jp",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick pressed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"jr",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Joystick released",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"kp",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Key pressed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"kr",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Key released",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"mp",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Mouse pressed",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"mr",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Mouse released",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"q",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Quit",
        },
    },
    LoveApiTypedItem {
        type_name: r"Event",
        item: LuaItem {
            label: r"f",
            kind: LuaItemKind::Value,
            detail: r"Event",
            documentation: r"Window focus gained or lost",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileDecoder",
        item: LuaItem {
            label: r"file",
            kind: LuaItemKind::Value,
            detail: r"FileDecoder",
            documentation: r"The data is unencoded.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileDecoder",
        item: LuaItem {
            label: r"base64",
            kind: LuaItemKind::Value,
            detail: r"FileDecoder",
            documentation: r"The data is base64-encoded.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileMode",
        item: LuaItem {
            label: r"r",
            kind: LuaItemKind::Value,
            detail: r"FileMode",
            documentation: r"Open a file for read.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileMode",
        item: LuaItem {
            label: r"w",
            kind: LuaItemKind::Value,
            detail: r"FileMode",
            documentation: r"Open a file for write.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileMode",
        item: LuaItem {
            label: r"a",
            kind: LuaItemKind::Value,
            detail: r"FileMode",
            documentation: r"Open a file for append.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileMode",
        item: LuaItem {
            label: r"c",
            kind: LuaItemKind::Value,
            detail: r"FileMode",
            documentation: r"Do not open a file (represents a closed file.)",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileType",
        item: LuaItem {
            label: r"file",
            kind: LuaItemKind::Value,
            detail: r"FileType",
            documentation: r"Regular file.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileType",
        item: LuaItem {
            label: r"directory",
            kind: LuaItemKind::Value,
            detail: r"FileType",
            documentation: r"Directory.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileType",
        item: LuaItem {
            label: r"symlink",
            kind: LuaItemKind::Value,
            detail: r"FileType",
            documentation: r"Symbolic link.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FileType",
        item: LuaItem {
            label: r"other",
            kind: LuaItemKind::Value,
            detail: r"FileType",
            documentation: r"Something completely different like a device.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FilterMode",
        item: LuaItem {
            label: r"linear",
            kind: LuaItemKind::Value,
            detail: r"FilterMode",
            documentation: r"Scale image with linear interpolation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FilterMode",
        item: LuaItem {
            label: r"nearest",
            kind: LuaItemKind::Value,
            detail: r"FilterMode",
            documentation: r"Scale image with nearest neighbor interpolation.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FilterType",
        item: LuaItem {
            label: r"lowpass",
            kind: LuaItemKind::Value,
            detail: r"FilterType",
            documentation: r"Low-pass filter. High frequency sounds are attenuated.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FilterType",
        item: LuaItem {
            label: r"highpass",
            kind: LuaItemKind::Value,
            detail: r"FilterType",
            documentation: r"High-pass filter. Low frequency sounds are attenuated.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FilterType",
        item: LuaItem {
            label: r"bandpass",
            kind: LuaItemKind::Value,
            detail: r"FilterType",
            documentation: r"Band-pass filter. Both high and low frequency sounds are attenuated based on the given parameters.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FullscreenType",
        item: LuaItem {
            label: r"desktop",
            kind: LuaItemKind::Value,
            detail: r"FullscreenType",
            documentation: r"Sometimes known as borderless fullscreen windowed mode. A borderless screen-sized window is created which sits on top of all desktop UI elements. The window is automatically resized to match the dimensions of the desktop, and its size cannot be changed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FullscreenType",
        item: LuaItem {
            label: r"exclusive",
            kind: LuaItemKind::Value,
            detail: r"FullscreenType",
            documentation: r"Standard exclusive-fullscreen mode. Changes the display mode (actual resolution) of the monitor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"FullscreenType",
        item: LuaItem {
            label: r"normal",
            kind: LuaItemKind::Value,
            detail: r"FullscreenType",
            documentation: r"Standard exclusive-fullscreen mode. Changes the display mode (actual resolution) of the monitor.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadAxis",
        item: LuaItem {
            label: r"leftx",
            kind: LuaItemKind::Value,
            detail: r"GamepadAxis",
            documentation: r"The x-axis of the left thumbstick.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadAxis",
        item: LuaItem {
            label: r"lefty",
            kind: LuaItemKind::Value,
            detail: r"GamepadAxis",
            documentation: r"The y-axis of the left thumbstick.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadAxis",
        item: LuaItem {
            label: r"rightx",
            kind: LuaItemKind::Value,
            detail: r"GamepadAxis",
            documentation: r"The x-axis of the right thumbstick.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadAxis",
        item: LuaItem {
            label: r"righty",
            kind: LuaItemKind::Value,
            detail: r"GamepadAxis",
            documentation: r"The y-axis of the right thumbstick.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadAxis",
        item: LuaItem {
            label: r"triggerleft",
            kind: LuaItemKind::Value,
            detail: r"GamepadAxis",
            documentation: r"Left analog trigger.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadAxis",
        item: LuaItem {
            label: r"triggerright",
            kind: LuaItemKind::Value,
            detail: r"GamepadAxis",
            documentation: r"Right analog trigger.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"a",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Bottom face button (A).",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"b",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Right face button (B).",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"x",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Left face button (X).",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"y",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Top face button (Y).",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"back",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Back button.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"guide",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Guide button.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"start",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Start button.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"leftstick",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Left stick click button.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"rightstick",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Right stick click button.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"leftshoulder",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Left bumper.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"rightshoulder",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"Right bumper.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"dpup",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"D-pad up.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"dpdown",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"D-pad down.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"dpleft",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"D-pad left.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GamepadButton",
        item: LuaItem {
            label: r"dpright",
            kind: LuaItemKind::Value,
            detail: r"GamepadButton",
            documentation: r"D-pad right.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsFeature",
        item: LuaItem {
            label: r"clampzero",
            kind: LuaItemKind::Value,
            detail: r"GraphicsFeature",
            documentation: r#"Whether the "clampzero" WrapMode is supported."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsFeature",
        item: LuaItem {
            label: r"lighten",
            kind: LuaItemKind::Value,
            detail: r"GraphicsFeature",
            documentation: r#"Whether the "lighten" and "darken" BlendModes are supported."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsFeature",
        item: LuaItem {
            label: r"multicanvasformats",
            kind: LuaItemKind::Value,
            detail: r"GraphicsFeature",
            documentation: r"Whether multiple formats can be used in the same love.graphics.setCanvas call.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsFeature",
        item: LuaItem {
            label: r"glsl3",
            kind: LuaItemKind::Value,
            detail: r"GraphicsFeature",
            documentation: r"Whether GLSL 3 Shaders can be used.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsFeature",
        item: LuaItem {
            label: r"instancing",
            kind: LuaItemKind::Value,
            detail: r"GraphicsFeature",
            documentation: r"Whether mesh instancing is supported.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsFeature",
        item: LuaItem {
            label: r"fullnpot",
            kind: LuaItemKind::Value,
            detail: r"GraphicsFeature",
            documentation: r"Whether textures with non-power-of-two dimensions can use mipmapping and the 'repeat' WrapMode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsFeature",
        item: LuaItem {
            label: r"pixelshaderhighp",
            kind: LuaItemKind::Value,
            detail: r"GraphicsFeature",
            documentation: r#"Whether pixel shaders can use "highp" 32 bit floating point numbers (as opposed to just 16 bit or lower precision)."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsFeature",
        item: LuaItem {
            label: r"shaderderivatives",
            kind: LuaItemKind::Value,
            detail: r"GraphicsFeature",
            documentation: r"Whether shaders can use the dFdx, dFdy, and fwidth functions for computing derivatives.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsLimit",
        item: LuaItem {
            label: r"pointsize",
            kind: LuaItemKind::Value,
            detail: r"GraphicsLimit",
            documentation: r"The maximum size of points.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsLimit",
        item: LuaItem {
            label: r"texturesize",
            kind: LuaItemKind::Value,
            detail: r"GraphicsLimit",
            documentation: r"The maximum width or height of Images and Canvases.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsLimit",
        item: LuaItem {
            label: r"multicanvas",
            kind: LuaItemKind::Value,
            detail: r"GraphicsLimit",
            documentation: r"The maximum number of simultaneously active canvases (via love.graphics.setCanvas.)",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsLimit",
        item: LuaItem {
            label: r"canvasmsaa",
            kind: LuaItemKind::Value,
            detail: r"GraphicsLimit",
            documentation: r"The maximum number of antialiasing samples for a Canvas.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsLimit",
        item: LuaItem {
            label: r"texturelayers",
            kind: LuaItemKind::Value,
            detail: r"GraphicsLimit",
            documentation: r"The maximum number of layers in an Array texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsLimit",
        item: LuaItem {
            label: r"volumetexturesize",
            kind: LuaItemKind::Value,
            detail: r"GraphicsLimit",
            documentation: r"The maximum width, height, or depth of a Volume texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsLimit",
        item: LuaItem {
            label: r"cubetexturesize",
            kind: LuaItemKind::Value,
            detail: r"GraphicsLimit",
            documentation: r"The maximum width or height of a Cubemap texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"GraphicsLimit",
        item: LuaItem {
            label: r"anisotropy",
            kind: LuaItemKind::Value,
            detail: r"GraphicsLimit",
            documentation: r"The maximum amount of anisotropic filtering. Texture:setMipmapFilter internally clamps the given anisotropy value to the system's limit.",
        },
    },
    LoveApiTypedItem {
        type_name: r"HashFunction",
        item: LuaItem {
            label: r"md5",
            kind: LuaItemKind::Value,
            detail: r"HashFunction",
            documentation: r"MD5 hash algorithm (16 bytes).",
        },
    },
    LoveApiTypedItem {
        type_name: r"HashFunction",
        item: LuaItem {
            label: r"sha1",
            kind: LuaItemKind::Value,
            detail: r"HashFunction",
            documentation: r"SHA1 hash algorithm (20 bytes).",
        },
    },
    LoveApiTypedItem {
        type_name: r"HashFunction",
        item: LuaItem {
            label: r"sha224",
            kind: LuaItemKind::Value,
            detail: r"HashFunction",
            documentation: r"SHA2 hash algorithm with message digest size of 224 bits (28 bytes).",
        },
    },
    LoveApiTypedItem {
        type_name: r"HashFunction",
        item: LuaItem {
            label: r"sha256",
            kind: LuaItemKind::Value,
            detail: r"HashFunction",
            documentation: r"SHA2 hash algorithm with message digest size of 256 bits (32 bytes).",
        },
    },
    LoveApiTypedItem {
        type_name: r"HashFunction",
        item: LuaItem {
            label: r"sha384",
            kind: LuaItemKind::Value,
            detail: r"HashFunction",
            documentation: r"SHA2 hash algorithm with message digest size of 384 bits (48 bytes).",
        },
    },
    LoveApiTypedItem {
        type_name: r"HashFunction",
        item: LuaItem {
            label: r"sha512",
            kind: LuaItemKind::Value,
            detail: r"HashFunction",
            documentation: r"SHA2 hash algorithm with message digest size of 512 bits (64 bytes).",
        },
    },
    LoveApiTypedItem {
        type_name: r"HintingMode",
        item: LuaItem {
            label: r"normal",
            kind: LuaItemKind::Value,
            detail: r"HintingMode",
            documentation: r"Default hinting. Should be preferred for typical antialiased fonts.",
        },
    },
    LoveApiTypedItem {
        type_name: r"HintingMode",
        item: LuaItem {
            label: r"light",
            kind: LuaItemKind::Value,
            detail: r"HintingMode",
            documentation: r"Results in fuzzier text but can sometimes preserve the original glyph shapes of the text better than normal hinting.",
        },
    },
    LoveApiTypedItem {
        type_name: r"HintingMode",
        item: LuaItem {
            label: r"mono",
            kind: LuaItemKind::Value,
            detail: r"HintingMode",
            documentation: r"Results in aliased / unsmoothed text with either full opacity or completely transparent pixels. Should be used when antialiasing is not desired for the font.",
        },
    },
    LoveApiTypedItem {
        type_name: r"HintingMode",
        item: LuaItem {
            label: r"none",
            kind: LuaItemKind::Value,
            detail: r"HintingMode",
            documentation: r"Disables hinting for the font. Results in fuzzier text.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageFormat",
        item: LuaItem {
            label: r"tga",
            kind: LuaItemKind::Value,
            detail: r"ImageFormat",
            documentation: r"Targa image format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageFormat",
        item: LuaItem {
            label: r"png",
            kind: LuaItemKind::Value,
            detail: r"ImageFormat",
            documentation: r"PNG image format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageFormat",
        item: LuaItem {
            label: r"jpg",
            kind: LuaItemKind::Value,
            detail: r"ImageFormat",
            documentation: r"JPG image format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ImageFormat",
        item: LuaItem {
            label: r"bmp",
            kind: LuaItemKind::Value,
            detail: r"ImageFormat",
            documentation: r"BMP image format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"IndexDataType",
        item: LuaItem {
            label: r"uint16",
            kind: LuaItemKind::Value,
            detail: r"IndexDataType",
            documentation: r"The vertex map is array of unsigned word (16-bit).",
        },
    },
    LoveApiTypedItem {
        type_name: r"IndexDataType",
        item: LuaItem {
            label: r"uint32",
            kind: LuaItemKind::Value,
            detail: r"IndexDataType",
            documentation: r"The vertex map is array of unsigned dword (32-bit).",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"distance",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A DistanceJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"friction",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A FrictionJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"gear",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A GearJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"mouse",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A MouseJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"prismatic",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A PrismaticJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"pulley",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A PulleyJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"revolute",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A RevoluteJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"rope",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A RopeJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JointType",
        item: LuaItem {
            label: r"weld",
            kind: LuaItemKind::Value,
            detail: r"JointType",
            documentation: r"A WeldJoint.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"c",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Centered",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"d",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Down",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"l",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Left",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"ld",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Left+Down",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"lu",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Left+Up",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"r",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Right",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"rd",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Right+Down",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"ru",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Right+Up",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickHat",
        item: LuaItem {
            label: r"u",
            kind: LuaItemKind::Value,
            detail: r"JoystickHat",
            documentation: r"Up",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickInputType",
        item: LuaItem {
            label: r"axis",
            kind: LuaItemKind::Value,
            detail: r"JoystickInputType",
            documentation: r"Analog axis.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickInputType",
        item: LuaItem {
            label: r"button",
            kind: LuaItemKind::Value,
            detail: r"JoystickInputType",
            documentation: r"Button.",
        },
    },
    LoveApiTypedItem {
        type_name: r"JoystickInputType",
        item: LuaItem {
            label: r"hat",
            kind: LuaItemKind::Value,
            detail: r"JoystickInputType",
            documentation: r"8-direction hat value.",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"a",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The A key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"b",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The B key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"c",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The C key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"d",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The D key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"e",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The E key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The F key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"g",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The G key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"h",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The H key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"i",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The I key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"j",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The J key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"k",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The K key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"l",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The L key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"m",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The M key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"n",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The N key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"o",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The O key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"p",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The P key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"q",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The Q key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"r",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The R key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"s",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The S key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"t",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The T key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"u",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The U key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"v",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The V key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"w",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The W key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"x",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The X key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"y",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The Y key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"z",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The Z key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"0",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The zero key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"1",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The one key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"2",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The two key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"3",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The three key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"4",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The four key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"5",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The five key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"6",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The six key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"7",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The seven key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"8",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The eight key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"9",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The nine key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"space",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Space key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"!",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Exclamation mark key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r#"""#,
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Double quote key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"#",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Hash key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"$",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Dollar key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"&",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Ampersand key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"'",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Single quote key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"(",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Left parenthesis key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r")",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Right parenthesis key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"*",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Asterisk key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"+",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Plus key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r",",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Comma key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"-",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Hyphen-minus key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r".",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Full stop key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"/",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Slash key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r":",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Colon key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r";",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Semicolon key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"<",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Less-than key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"=",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Equal key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r">",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Greater-than key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"?",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Question mark key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"@",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"At sign key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"[",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Left square bracket key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"\",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Backslash key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"]",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Right square bracket key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"^",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Caret key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"_",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Underscore key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"`",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Grave accent key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp0",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad zero key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp1",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad one key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp2",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad two key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp3",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad three key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp4",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad four key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp5",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad five key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp6",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad six key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp7",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad seven key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp8",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad eight key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp9",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad nine key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp.",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad decimal point key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp/",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad division key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp*",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad multiplication key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp-",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad substraction key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp+",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad addition key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kpenter",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad enter key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"kp=",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The numpad equals key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"up",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Up cursor key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"down",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Down cursor key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"right",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Right cursor key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"left",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Left cursor key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"home",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Home key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"end",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"End key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"pageup",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Page up key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"pagedown",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Page down key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"insert",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Insert key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"backspace",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Backspace key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"tab",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Tab key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"clear",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Clear key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"return",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Return key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"delete",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Delete key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f1",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 1st function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f2",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 2nd function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f3",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 3rd function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f4",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 4th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f5",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 5th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f6",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 6th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f7",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 7th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f8",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 8th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f9",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 9th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f10",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 10th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f11",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 11th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f12",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 12th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f13",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 13th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f14",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 14th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"f15",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"The 15th function key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"numlock",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Num-lock key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"capslock",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Caps-lock key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"scrollock",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Scroll-lock key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"rshift",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Right shift key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"lshift",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Left shift key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"rctrl",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Right control key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"lctrl",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Left control key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"ralt",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Right alt key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"lalt",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Left alt key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"rmeta",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Right meta key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"lmeta",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Left meta key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"lsuper",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Left super key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"rsuper",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Right super key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"mode",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Mode key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"compose",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Compose key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"pause",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Pause key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"escape",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Escape key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"help",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Help key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"print",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Print key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"sysreq",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"System request key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"break",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Break key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"menu",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Menu key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"power",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Power key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"euro",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Euro (&euro;) key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"undo",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Undo key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"www",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"WWW key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"mail",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Mail key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"calculator",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Calculator key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"appsearch",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Application search key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"apphome",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Application home key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"appback",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Application back key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"appforward",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Application forward key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"apprefresh",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Application refresh key",
        },
    },
    LoveApiTypedItem {
        type_name: r"KeyConstant",
        item: LuaItem {
            label: r"appbookmarks",
            kind: LuaItemKind::Value,
            detail: r"KeyConstant",
            documentation: r"Application bookmarks key",
        },
    },
    LoveApiTypedItem {
        type_name: r"LineJoin",
        item: LuaItem {
            label: r"miter",
            kind: LuaItemKind::Value,
            detail: r"LineJoin",
            documentation: r"The ends of the line segments beveled in an angle so that they join seamlessly.",
        },
    },
    LoveApiTypedItem {
        type_name: r"LineJoin",
        item: LuaItem {
            label: r"none",
            kind: LuaItemKind::Value,
            detail: r"LineJoin",
            documentation: r"No cap applied to the ends of the line segments.",
        },
    },
    LoveApiTypedItem {
        type_name: r"LineJoin",
        item: LuaItem {
            label: r"bevel",
            kind: LuaItemKind::Value,
            detail: r"LineJoin",
            documentation: r"Flattens the point where line segments join together.",
        },
    },
    LoveApiTypedItem {
        type_name: r"LineStyle",
        item: LuaItem {
            label: r"rough",
            kind: LuaItemKind::Value,
            detail: r"LineStyle",
            documentation: r"Draw rough lines.",
        },
    },
    LoveApiTypedItem {
        type_name: r"LineStyle",
        item: LuaItem {
            label: r"smooth",
            kind: LuaItemKind::Value,
            detail: r"LineStyle",
            documentation: r"Draw smooth lines.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MatrixLayout",
        item: LuaItem {
            label: r"row",
            kind: LuaItemKind::Value,
            detail: r"MatrixLayout",
            documentation: r"The matrix is row-major:",
        },
    },
    LoveApiTypedItem {
        type_name: r"MatrixLayout",
        item: LuaItem {
            label: r"column",
            kind: LuaItemKind::Value,
            detail: r"MatrixLayout",
            documentation: r"The matrix is column-major:",
        },
    },
    LoveApiTypedItem {
        type_name: r"MeshDrawMode",
        item: LuaItem {
            label: r"fan",
            kind: LuaItemKind::Value,
            detail: r"MeshDrawMode",
            documentation: r#"The vertices create a "fan" shape with the first vertex acting as the hub point. Can be easily used to draw simple convex polygons."#,
        },
    },
    LoveApiTypedItem {
        type_name: r"MeshDrawMode",
        item: LuaItem {
            label: r"strip",
            kind: LuaItemKind::Value,
            detail: r"MeshDrawMode",
            documentation: r"The vertices create a series of connected triangles using vertices 1, 2, 3, then 3, 2, 4 (note the order), then 3, 4, 5, and so on.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MeshDrawMode",
        item: LuaItem {
            label: r"triangles",
            kind: LuaItemKind::Value,
            detail: r"MeshDrawMode",
            documentation: r"The vertices create unconnected triangles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MeshDrawMode",
        item: LuaItem {
            label: r"points",
            kind: LuaItemKind::Value,
            detail: r"MeshDrawMode",
            documentation: r"The vertices are drawn as unconnected points (see love.graphics.setPointSize.)",
        },
    },
    LoveApiTypedItem {
        type_name: r"MessageBoxType",
        item: LuaItem {
            label: r"info",
            kind: LuaItemKind::Value,
            detail: r"MessageBoxType",
            documentation: r"Informational dialog.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MessageBoxType",
        item: LuaItem {
            label: r"warning",
            kind: LuaItemKind::Value,
            detail: r"MessageBoxType",
            documentation: r"Warning dialog.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MessageBoxType",
        item: LuaItem {
            label: r"error",
            kind: LuaItemKind::Value,
            detail: r"MessageBoxType",
            documentation: r"Error dialog.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MipmapMode",
        item: LuaItem {
            label: r"none",
            kind: LuaItemKind::Value,
            detail: r"MipmapMode",
            documentation: r"The Canvas has no mipmaps.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MipmapMode",
        item: LuaItem {
            label: r"auto",
            kind: LuaItemKind::Value,
            detail: r"MipmapMode",
            documentation: r"The Canvas has mipmaps. love.graphics.setCanvas can be used to render to a specific mipmap level, or Canvas:generateMipmaps can (re-)compute all mipmap levels based on the base level.",
        },
    },
    LoveApiTypedItem {
        type_name: r"MipmapMode",
        item: LuaItem {
            label: r"manual",
            kind: LuaItemKind::Value,
            detail: r"MipmapMode",
            documentation: r"The Canvas has mipmaps, and all mipmap levels will automatically be recomputed when switching away from the Canvas with love.graphics.setCanvas.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleInsertMode",
        item: LuaItem {
            label: r"top",
            kind: LuaItemKind::Value,
            detail: r"ParticleInsertMode",
            documentation: r"Particles are inserted at the top of the ParticleSystem's list of particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleInsertMode",
        item: LuaItem {
            label: r"bottom",
            kind: LuaItemKind::Value,
            detail: r"ParticleInsertMode",
            documentation: r"Particles are inserted at the bottom of the ParticleSystem's list of particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ParticleInsertMode",
        item: LuaItem {
            label: r"random",
            kind: LuaItemKind::Value,
            detail: r"ParticleInsertMode",
            documentation: r"Particles are inserted at random positions in the ParticleSystem's list of particles.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"unknown",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Indicates unknown pixel format, used internally.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"normal",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Alias for rgba8, or srgba8 if gamma-correct rendering is enabled.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"hdr",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"A format suitable for high dynamic range content - an alias for the rgba16f format, normally.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"r8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Single-channel (red component) format (8 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rg8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Two channels (red and green components) with 8 bits per channel (16 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rgba8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"8 bits per channel (32 bpp) RGBA. Color channel values range from 0-255 (0-1 in shaders).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"srgba8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"gamma-correct version of rgba8.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"r16",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Single-channel (red component) format (16 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rg16",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Two channels (red and green components) with 16 bits per channel (32 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rgba16",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"16 bits per channel (64 bpp) RGBA. Color channel values range from 0-65535 (0-1 in shaders).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"r16f",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Floating point single-channel format (16 bpp). Color values can range from [-65504, +65504].",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rg16f",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Floating point two-channel format with 16 bits per channel (32 bpp). Color values can range from [-65504, +65504].",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rgba16f",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Floating point RGBA with 16 bits per channel (64 bpp). Color values can range from [-65504, +65504].",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"r32f",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Floating point single-channel format (32 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rg32f",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Floating point two-channel format with 32 bits per channel (64 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rgba32f",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Floating point RGBA with 32 bits per channel (128 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"la8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Same as rg8, but accessed as (L, L, L, A)",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rgba4",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"4 bits per channel (16 bpp) RGBA.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rgb5a1",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"RGB with 5 bits each, and a 1-bit alpha channel (16 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rgb565",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"RGB with 5, 6, and 5 bits each, respectively (16 bpp). There is no alpha channel in this format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rgb10a2",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"RGB with 10 bits per channel, and a 2-bit alpha channel (32 bpp).",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"rg11b10f",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"Floating point RGB with 11 bits in the red and green channels, and 10 bits in the blue channel (32 bpp). There is no alpha channel. Color values can range from [0, +65024].",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"stencil8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"No depth buffer and 8-bit stencil buffer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"depth16",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"16-bit depth buffer and no stencil buffer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"depth24",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"24-bit depth buffer and no stencil buffer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"depth32f",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"32-bit float depth buffer and no stencil buffer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"depth24stencil8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"24-bit depth buffer and 8-bit stencil buffer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"depth32fstencil8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"32-bit float depth buffer and 8-bit stencil buffer.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"DXT1",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The DXT1 format. RGB data at 4 bits per pixel (compared to 32 bits for ImageData and regular Images.) Suitable for fully opaque images on desktop systems.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"DXT3",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The DXT3 format. RGBA data at 8 bits per pixel. Smooth variations in opacity do not mix well with this format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"DXT5",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The DXT5 format. RGBA data at 8 bits per pixel. Recommended for images with varying opacity on desktop systems.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"BC4",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The BC4 format (also known as 3Dc+ or ATI1.) Stores just the red channel, at 4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"BC4s",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The signed variant of the BC4 format. Same as above but pixel values in the texture are in the range of 1 instead of 1 in shaders.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"BC5",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The BC5 format (also known as 3Dc or ATI2.) Stores red and green channels at 8 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"BC5s",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The signed variant of the BC5 format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"BC6h",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The BC6H format. Stores half-precision floating-point RGB data in the range of 65504 at 8 bits per pixel. Suitable for HDR images on desktop systems.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"BC6hs",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The signed variant of the BC6H format. Stores RGB data in the range of +65504.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"BC7",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The BC7 format (also known as BPTC.) Stores RGB or RGBA data at 8 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ETC1",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The ETC1 format. RGB data at 4 bits per pixel. Suitable for fully opaque images on older Android devices.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ETC2rgb",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The RGB variant of the ETC2 format. RGB data at 4 bits per pixel. Suitable for fully opaque images on newer mobile devices.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ETC2rgba",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The RGBA variant of the ETC2 format. RGBA data at 8 bits per pixel. Recommended for images with varying opacity on newer mobile devices.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ETC2rgba1",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The RGBA variant of the ETC2 format where pixels are either fully transparent or fully opaque. RGBA data at 4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"EACr",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The single-channel variant of the EAC format. Stores just the red channel, at 4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"EACrs",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The signed single-channel variant of the EAC format. Same as above but pixel values in the texture are in the range of 1 instead of 1 in shaders.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"EACrg",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The two-channel variant of the EAC format. Stores red and green channels at 8 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"EACrgs",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The signed two-channel variant of the EAC format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"PVR1rgb2",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 2 bit per pixel RGB variant of the PVRTC1 format. Stores RGB data at 2 bits per pixel. Textures compressed with PVRTC1 formats must be square and power-of-two sized.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"PVR1rgb4",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 4 bit per pixel RGB variant of the PVRTC1 format. Stores RGB data at 4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"PVR1rgba2",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 2 bit per pixel RGBA variant of the PVRTC1 format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"PVR1rgba4",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 4 bit per pixel RGBA variant of the PVRTC1 format.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC4x4",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 4x4 pixels per block variant of the ASTC format. RGBA data at 8 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC5x4",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 5x4 pixels per block variant of the ASTC format. RGBA data at 6.4 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC5x5",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 5x5 pixels per block variant of the ASTC format. RGBA data at 5.12 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC6x5",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 6x5 pixels per block variant of the ASTC format. RGBA data at 4.27 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC6x6",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 6x6 pixels per block variant of the ASTC format. RGBA data at 3.56 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC8x5",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 8x5 pixels per block variant of the ASTC format. RGBA data at 3.2 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC8x6",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 8x6 pixels per block variant of the ASTC format. RGBA data at 2.67 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC8x8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 8x8 pixels per block variant of the ASTC format. RGBA data at 2 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC10x5",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 10x5 pixels per block variant of the ASTC format. RGBA data at 2.56 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC10x6",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 10x6 pixels per block variant of the ASTC format. RGBA data at 2.13 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC10x8",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 10x8 pixels per block variant of the ASTC format. RGBA data at 1.6 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC10x10",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 10x10 pixels per block variant of the ASTC format. RGBA data at 1.28 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC12x10",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 12x10 pixels per block variant of the ASTC format. RGBA data at 1.07 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PixelFormat",
        item: LuaItem {
            label: r"ASTC12x12",
            kind: LuaItemKind::Value,
            detail: r"PixelFormat",
            documentation: r"The 12x12 pixels per block variant of the ASTC format. RGBA data at 0.89 bits per pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PowerState",
        item: LuaItem {
            label: r"unknown",
            kind: LuaItemKind::Value,
            detail: r"PowerState",
            documentation: r"Cannot determine power status.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PowerState",
        item: LuaItem {
            label: r"battery",
            kind: LuaItemKind::Value,
            detail: r"PowerState",
            documentation: r"Not plugged in, running on a battery.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PowerState",
        item: LuaItem {
            label: r"nobattery",
            kind: LuaItemKind::Value,
            detail: r"PowerState",
            documentation: r"Plugged in, no battery available.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PowerState",
        item: LuaItem {
            label: r"charging",
            kind: LuaItemKind::Value,
            detail: r"PowerState",
            documentation: r"Plugged in, charging battery.",
        },
    },
    LoveApiTypedItem {
        type_name: r"PowerState",
        item: LuaItem {
            label: r"charged",
            kind: LuaItemKind::Value,
            detail: r"PowerState",
            documentation: r"Plugged in, battery is fully charged.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"a",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'A' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"b",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'B' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"c",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'C' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"d",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'D' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"e",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'E' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'F' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"g",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'G' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"h",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'H' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"i",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'I' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"j",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'J' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"k",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'K' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"l",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'L' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"m",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'M' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"n",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'N' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"o",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'O' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"p",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'P' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"q",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'Q' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"r",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'R' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"s",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'S' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"t",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'T' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"u",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'U' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"v",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'V' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"w",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'W' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"x",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'X' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"y",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'Y' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"z",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'Z' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"1",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '1' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"2",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '2' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"3",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '3' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"4",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '4' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"5",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '5' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"6",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '6' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"7",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '7' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"8",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '8' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"9",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '9' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"0",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The '0' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"return",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'return' / 'enter' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"escape",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'escape' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"backspace",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'backspace' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"tab",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'tab' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"space",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The spacebar on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"-",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The minus key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"=",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The equals key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"[",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The left-bracket key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"]",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The right-bracket key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"\",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The backslash key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"nonus#",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The non-U.S. hash scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r";",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The semicolon key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"'",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The apostrophe key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"`",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The back-tick / grave key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r",",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The comma key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r".",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The period key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"/",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The forward-slash key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"capslock",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The capslock key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f1",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F1 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f2",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F2 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f3",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F3 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f4",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F4 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f5",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F5 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f6",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F6 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f7",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F7 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f8",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F8 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f9",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F9 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f10",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F10 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f11",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F11 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f12",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F12 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f13",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F13 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f14",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F14 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f15",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F15 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f16",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F16 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f17",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F17 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f18",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F18 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f19",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F19 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f20",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F20 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f21",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F21 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f22",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F22 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f23",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F23 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"f24",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The F24 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lctrl",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The left control key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lshift",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The left shift key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lalt",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The left alt / option key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lgui",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The left GUI (command / windows / super) key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"rctrl",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The right control key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"rshift",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The right shift key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"ralt",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The right alt / option key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"rgui",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The right GUI (command / windows / super) key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"printscreen",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The printscreen key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"scrolllock",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The scroll-lock key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"pause",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The pause key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"insert",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The insert key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"home",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The home key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"numlock",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The numlock / clear key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"pageup",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The page-up key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"delete",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The forward-delete key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"end",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The end key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"pagedown",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The page-down key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"right",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The right-arrow key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"left",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The left-arrow key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"down",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The down-arrow key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"up",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The up-arrow key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"nonusbackslash",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The non-U.S. backslash scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"application",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The application key on an American layout. Windows contextual menu, compose key.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"execute",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'execute' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"help",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'help' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"menu",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'menu' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"select",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'select' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"stop",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'stop' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"again",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'again' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"undo",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'undo' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"cut",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'cut' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"copy",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'copy' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"paste",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'paste' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"find",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'find' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp/",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad forward-slash key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp*",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '*' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp-",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad minus key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp+",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad plus key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp=",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad equals key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kpenter",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad enter key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp1",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '1' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp2",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '2' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp3",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '3' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp4",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '4' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp5",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '5' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp6",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '6' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp7",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '7' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp8",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '8' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp9",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '9' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp0",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad '0' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp.",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad period key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international1",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 1st international key on an American layout. Used on Asian keyboards.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international2",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 2nd international key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international3",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 3rd international  key on an American layout. Yen.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international4",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 4th international key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international5",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 5th international key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international6",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 6th international key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international7",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 7th international key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international8",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 8th international key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"international9",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 9th international key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lang1",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"Hangul/English toggle scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lang2",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"Hanja conversion scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lang3",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"Katakana scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lang4",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"Hiragana scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"lang5",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"Zenkaku/Hankaku scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"mute",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The mute key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"volumeup",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The volume up key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"volumedown",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The volume down key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"audionext",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The audio next track key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"audioprev",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The audio previous track key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"audiostop",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The audio stop key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"audioplay",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The audio play key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"audiomute",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The audio mute key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"mediaselect",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The media select key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"www",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'WWW' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"mail",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The Mail key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"calculator",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The calculator key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"computer",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'computer' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"acsearch",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The AC Search key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"achome",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The AC Home key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"acback",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The AC Back key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"acforward",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The AC Forward key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"acstop",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"Th AC Stop key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"acrefresh",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The AC Refresh key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"acbookmarks",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The AC Bookmarks key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"power",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The system power scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"brightnessdown",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The brightness-down scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"brightnessup",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The brightness-up scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"displayswitch",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The display switch scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kbdillumtoggle",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keyboard illumination toggle scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kbdillumdown",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keyboard illumination down scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kbdillumup",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keyboard illumination up scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"eject",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The eject scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"sleep",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The system sleep scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"alterase",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The alt-erase key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"sysreq",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The sysreq key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"cancel",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'cancel' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"clear",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'clear' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"prior",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'prior' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"return2",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'return2' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"separator",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'separator' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"out",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'out' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"oper",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'oper' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"clearagain",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'clearagain' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"crsel",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'crsel' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"exsel",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'exsel' key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp00",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad 00 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"kp000",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The keypad 000 key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"thsousandsseparator",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The thousands-separator key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"decimalseparator",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The decimal separator key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"currencyunit",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The currency unit key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"currencysubunit",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The currency sub-unit key on an American layout.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"app1",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'app1' scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"app2",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"The 'app2' scancode.",
        },
    },
    LoveApiTypedItem {
        type_name: r"Scancode",
        item: LuaItem {
            label: r"unknown",
            kind: LuaItemKind::Value,
            detail: r"Scancode",
            documentation: r"An unknown key.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ShapeType",
        item: LuaItem {
            label: r"circle",
            kind: LuaItemKind::Value,
            detail: r"ShapeType",
            documentation: r"The Shape is a CircleShape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ShapeType",
        item: LuaItem {
            label: r"polygon",
            kind: LuaItemKind::Value,
            detail: r"ShapeType",
            documentation: r"The Shape is a PolygonShape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ShapeType",
        item: LuaItem {
            label: r"edge",
            kind: LuaItemKind::Value,
            detail: r"ShapeType",
            documentation: r"The Shape is a EdgeShape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"ShapeType",
        item: LuaItem {
            label: r"chain",
            kind: LuaItemKind::Value,
            detail: r"ShapeType",
            documentation: r"The Shape is a ChainShape.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SourceType",
        item: LuaItem {
            label: r"static",
            kind: LuaItemKind::Value,
            detail: r"SourceType",
            documentation: r"The whole audio is decoded.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SourceType",
        item: LuaItem {
            label: r"stream",
            kind: LuaItemKind::Value,
            detail: r"SourceType",
            documentation: r"The audio is decoded in chunks when needed.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SourceType",
        item: LuaItem {
            label: r"queue",
            kind: LuaItemKind::Value,
            detail: r"SourceType",
            documentation: r"The audio must be manually queued by the user.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatchUsage",
        item: LuaItem {
            label: r"dynamic",
            kind: LuaItemKind::Value,
            detail: r"SpriteBatchUsage",
            documentation: r"The object's data will change occasionally during its lifetime. ",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatchUsage",
        item: LuaItem {
            label: r"static",
            kind: LuaItemKind::Value,
            detail: r"SpriteBatchUsage",
            documentation: r"The object will not be modified after initial sprites or vertices are added.",
        },
    },
    LoveApiTypedItem {
        type_name: r"SpriteBatchUsage",
        item: LuaItem {
            label: r"stream",
            kind: LuaItemKind::Value,
            detail: r"SpriteBatchUsage",
            documentation: r"The object data will always change between draws.",
        },
    },
    LoveApiTypedItem {
        type_name: r"StackType",
        item: LuaItem {
            label: r"transform",
            kind: LuaItemKind::Value,
            detail: r"StackType",
            documentation: r"The transformation stack (love.graphics.translate, love.graphics.rotate, etc.)",
        },
    },
    LoveApiTypedItem {
        type_name: r"StackType",
        item: LuaItem {
            label: r"all",
            kind: LuaItemKind::Value,
            detail: r"StackType",
            documentation: r"All love.graphics state, including transform state.",
        },
    },
    LoveApiTypedItem {
        type_name: r"StencilAction",
        item: LuaItem {
            label: r"replace",
            kind: LuaItemKind::Value,
            detail: r"StencilAction",
            documentation: r"The stencil value of a pixel will be replaced by the value specified in love.graphics.stencil, if any object touches the pixel.",
        },
    },
    LoveApiTypedItem {
        type_name: r"StencilAction",
        item: LuaItem {
            label: r"increment",
            kind: LuaItemKind::Value,
            detail: r"StencilAction",
            documentation: r"The stencil value of a pixel will be incremented by 1 for each object that touches the pixel. If the stencil value reaches 255 it will stay at 255.",
        },
    },
    LoveApiTypedItem {
        type_name: r"StencilAction",
        item: LuaItem {
            label: r"decrement",
            kind: LuaItemKind::Value,
            detail: r"StencilAction",
            documentation: r"The stencil value of a pixel will be decremented by 1 for each object that touches the pixel. If the stencil value reaches 0 it will stay at 0.",
        },
    },
    LoveApiTypedItem {
        type_name: r"StencilAction",
        item: LuaItem {
            label: r"incrementwrap",
            kind: LuaItemKind::Value,
            detail: r"StencilAction",
            documentation: r"The stencil value of a pixel will be incremented by 1 for each object that touches the pixel. If a stencil value of 255 is incremented it will be set to 0.",
        },
    },
    LoveApiTypedItem {
        type_name: r"StencilAction",
        item: LuaItem {
            label: r"decrementwrap",
            kind: LuaItemKind::Value,
            detail: r"StencilAction",
            documentation: r"The stencil value of a pixel will be decremented by 1 for each object that touches the pixel. If the stencil value of 0 is decremented it will be set to 255.",
        },
    },
    LoveApiTypedItem {
        type_name: r"StencilAction",
        item: LuaItem {
            label: r"invert",
            kind: LuaItemKind::Value,
            detail: r"StencilAction",
            documentation: r"The stencil value of a pixel will be bitwise-inverted for each object that touches the pixel. If a stencil value of 0 is inverted it will become 255.",
        },
    },
    LoveApiTypedItem {
        type_name: r"TextureType",
        item: LuaItem {
            label: r"2d",
            kind: LuaItemKind::Value,
            detail: r"TextureType",
            documentation: r"Regular 2D texture with width and height.",
        },
    },
    LoveApiTypedItem {
        type_name: r"TextureType",
        item: LuaItem {
            label: r"array",
            kind: LuaItemKind::Value,
            detail: r"TextureType",
            documentation: r"Several same-size 2D textures organized into a single object. Similar to a texture atlas / sprite sheet, but avoids sprite bleeding and other issues.",
        },
    },
    LoveApiTypedItem {
        type_name: r"TextureType",
        item: LuaItem {
            label: r"cube",
            kind: LuaItemKind::Value,
            detail: r"TextureType",
            documentation: r"Cubemap texture with 6 faces. Requires a custom shader (and Shader:send) to use. Sampling from a cube texture in a shader takes a 3D direction vector instead of a texture coordinate.",
        },
    },
    LoveApiTypedItem {
        type_name: r"TextureType",
        item: LuaItem {
            label: r"volume",
            kind: LuaItemKind::Value,
            detail: r"TextureType",
            documentation: r"3D texture with width, height, and depth. Requires a custom shader to use. Volume textures can have texture filtering applied along the 3rd axis.",
        },
    },
    LoveApiTypedItem {
        type_name: r"TimeUnit",
        item: LuaItem {
            label: r"seconds",
            kind: LuaItemKind::Value,
            detail: r"TimeUnit",
            documentation: r"Regular seconds.",
        },
    },
    LoveApiTypedItem {
        type_name: r"TimeUnit",
        item: LuaItem {
            label: r"samples",
            kind: LuaItemKind::Value,
            detail: r"TimeUnit",
            documentation: r"Audio samples.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VertexAttributeStep",
        item: LuaItem {
            label: r"pervertex",
            kind: LuaItemKind::Value,
            detail: r"VertexAttributeStep",
            documentation: r"The vertex attribute will have a unique value for each vertex in the Mesh.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VertexAttributeStep",
        item: LuaItem {
            label: r"perinstance",
            kind: LuaItemKind::Value,
            detail: r"VertexAttributeStep",
            documentation: r"The vertex attribute will have a unique value for each instance of the Mesh.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VertexWinding",
        item: LuaItem {
            label: r"cw",
            kind: LuaItemKind::Value,
            detail: r"VertexWinding",
            documentation: r"Clockwise.",
        },
    },
    LoveApiTypedItem {
        type_name: r"VertexWinding",
        item: LuaItem {
            label: r"ccw",
            kind: LuaItemKind::Value,
            detail: r"VertexWinding",
            documentation: r"Counter-clockwise.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WrapMode",
        item: LuaItem {
            label: r"clamp",
            kind: LuaItemKind::Value,
            detail: r"WrapMode",
            documentation: r"Clamp the texture. Appears only once. The area outside the texture's normal range is colored based on the edge pixels of the texture.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WrapMode",
        item: LuaItem {
            label: r"repeat",
            kind: LuaItemKind::Value,
            detail: r"WrapMode",
            documentation: r"Repeat the texture. Fills the whole available extent.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WrapMode",
        item: LuaItem {
            label: r"mirroredrepeat",
            kind: LuaItemKind::Value,
            detail: r"WrapMode",
            documentation: r"Repeat the texture, flipping it each time it repeats. May produce better visual results than the repeat mode when the texture doesn't seamlessly tile.",
        },
    },
    LoveApiTypedItem {
        type_name: r"WrapMode",
        item: LuaItem {
            label: r"clampzero",
            kind: LuaItemKind::Value,
            detail: r"WrapMode",
            documentation: r"Clamp the texture. Fills the area outside the texture's normal range with transparent black (or opaque black for textures with no alpha channel.)",
        },
    },
];
