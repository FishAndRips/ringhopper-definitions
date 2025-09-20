use alloc::string::String;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use serde_json::Value;

/// Contains all definitions.
#[derive(Default)]
pub struct ParsedDefinitions {
    /// Describes all definitions for structs, enums, and bitfields.
    pub objects: BTreeMap<String, NamedObject>,

    /// Describes all definitions for tag groups.
    pub groups: BTreeMap<String, TagGroup>,

    /// Describes all definitions for engines.
    pub engines: BTreeMap<String, Engine>
}

/// Allows you to query the size of an object.
pub trait SizeableObject {
    /// Get the size of the object in bytes
    fn size(&self, parsed_tag_data: &ParsedDefinitions) -> usize;
}

/// Describes a struct, enum, or bitfield type.
#[derive(Clone)]
pub enum NamedObject {
    /// Describes a struct type.
    Struct(Struct),

    /// Describes an enum type.
    Enum(Enum),

    /// Describes a bitfield type.
    Bitfield(Bitfield)
}

impl SizeableObject for NamedObject {
    fn size(&self, parsed_tag_data: &ParsedDefinitions) -> usize {
        match self {
            NamedObject::Bitfield(b) => b.size(parsed_tag_data),
            NamedObject::Enum(e) => e.size(parsed_tag_data),
            NamedObject::Struct(s) => s.size(parsed_tag_data)
        }
    }
}

impl NamedObject {
    /// Get the name of the object.
    pub fn name(&self) -> &str {
        match self {
            Self::Struct(s) => s.name.as_str(),
            Self::Enum(e) => e.name.as_str(),
            Self::Bitfield(b) => b.name.as_str(),
        }
    }
}

/// Describes a tag group.
pub struct TagGroup {
    /// Name of the tag group.
    pub name: String,

    /// Name of the base struct for this tag group.
    pub struct_name: String,

    /// Name of the tag group, itself, formatted for Rust enums.
    pub name_rust_enum: String,

    /// Supergroup, if any.
    pub supergroup: Option<String>,

    /// Engines that support this tag group.
    pub supported_engines: SupportedEngines,

    /// The version of the tag group (in a tag file).
    pub version: u16,

    /// The fourcc of the tag group.
    pub fourcc_binary: u32
}

/// Describes a struct, a composite block that potentially contains multiple fields.
#[derive(Clone)]
pub struct Struct {
    /// The name of the struct.
    pub name: String,

    /// All fields of the struct.
    pub fields: Vec<StructField>,

    /// The struct does not use tag dependencies, tag references, or tag data, and generating it
    /// in Rust can use bitwise Copy. This is assuming that all fields marked as `exclude` are
    /// excluded, too.
    pub is_const: bool,

    /// Flags for the struct, itself.
    pub flags: Flags,

    /// The final size of the struct in bytes
    pub size: usize
}

impl SizeableObject for Struct {
    fn size(&self, _: &ParsedDefinitions) -> usize {
        self.size
    }
}

impl Struct {
    fn set_offsets_and_verify_sizes(&mut self, parsed_tag_data: &ParsedDefinitions) {
        let expected_size = self.size;
        let mut real_size = 0;
        for f in &mut self.fields {
            f.relative_offset = real_size;
            real_size += f.size(parsed_tag_data);
        }
        assert_eq!(expected_size, real_size, "Size for {name} is incorrect (expected {expected_size}, got {real_size} instead)", name=self.name);
        assert_eq!(expected_size, self.size(parsed_tag_data), "size() is implemented wrong for {name} (expected {expected_size}, got {real_size} instead)", name=self.name);
    }
}

/// Describes a limit for something for a given field.
#[derive(PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub enum LimitType {
    /// Maximum allowed by the engine
    Engine(String),

    /// Maximum allowed by default
    Default,

    /// Maximum allowed by the editor
    Editor
}

/// Describes a field on a struct.
#[derive(Clone)]
pub struct StructField {
    /// Name of the field
    pub name: String,

    /// Name of the field, itself, formatted for Rust enums.
    pub name_rust_enum: String,

    /// Name of the field, itself, formatted for Rust fields.
    pub name_rust_field: String,

    /// Type of field
    pub field_type: StructFieldType,

    /// Is this a default value? If so, what are the default values for each field.
    pub default_value: Option<Vec<StaticValue>>,

    /// Number of fields
    pub count: FieldCount,

    /// Minimum value
    pub minimum: Option<StaticValue>,

    /// Maximum value
    pub maximum: Option<StaticValue>,

    /// Limits
    pub limit: Option<BTreeMap<LimitType, usize>>,

    /// Flags
    pub flags: Flags,

    /// Relative offset to the start of its structs.
    pub relative_offset: usize
}

impl SizeableObject for StructField {
    fn size(&self, parsed_tag_data: &ParsedDefinitions) -> usize {
        self.field_type.size(parsed_tag_data) * self.count.field_count()
    }
}

/// Describes a struct field.
#[derive(Clone)]
pub enum StructFieldType {
    /// This field is a tangible object with a meaning.
    Object(FieldObject),

    /// This field is just padding.
    Padding(usize),

    /// This field is an editor section rather than a real field.
    ///
    /// It is primarily for editors.
    EditorSection {
        /// Heading to use (the name).
        heading: String,

        /// The body of the editor section header.
        body: Option<String>
    }
}

impl SizeableObject for StructFieldType {
    fn size(&self, parsed_tag_data: &ParsedDefinitions) -> usize {
        match self {
            StructFieldType::Object(o) => o.size(parsed_tag_data),
            StructFieldType::Padding(u) => *u,
            StructFieldType::EditorSection { .. } => 0
        }
    }
}

/// Describes the number of values an object has.
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum FieldCount {
    /// A single field
    One,

    /// Expands to from/to
    Bounds,

    /// Array of multiple fields
    Array(usize)
}

impl FieldCount {
    fn field_count(&self) -> usize {
        match self {
            Self::One => 1,
            Self::Bounds => 2,
            Self::Array(u) => *u
        }
    }
}

/// Describes how an uninitialized field is handled.
pub struct DefaultBehavior {
    /// Default values for each field.
    ///
    /// For bounds, this is the \[from,to\]. For arrays, this is for each array element.
    pub default_value: Vec<StaticValue>,

    /// Default if the tag is being created
    pub default_on_creation: bool,

    /// Default if the value is equal to zero and being built into a cache file
    pub default_on_cache: bool
}

/// Describes a static value that is inside of the definitions, such as for default values.
#[derive(Debug, Clone)]
pub enum StaticValue {
    /// Describes a float value.
    Float(f32),

    /// Describes an unsigned integer value.
    Uint(u64),

    /// Describes an integer value.
    Int(i64),

    /// Describes a string value.
    String(String)
}

impl core::fmt::Display for StaticValue {
    fn fmt(&self, fmt: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StaticValue::String(s) => fmt.write_fmt(format_args!("\"{s}\"")),
            StaticValue::Uint(i) => fmt.write_fmt(format_args!("{i}")),
            StaticValue::Int(i) => fmt.write_fmt(format_args!("{i}")),
            StaticValue::Float(f) => fmt.write_fmt(format_args!("{f:0.032}f32"))
        }
    }
}

/// Describes a bitfield (a collection of booleans).
#[derive(Clone)]
pub struct Bitfield {
    /// Name of the bitfield
    pub name: String,

    /// Width in bits
    pub width: u8,

    /// Fields for the bitfield
    pub fields: Vec<Field>,

    /// Flags! Capture all of them to win!
    pub flags: Flags
}

impl SizeableObject for Bitfield {
    fn size(&self, _: &ParsedDefinitions) -> usize {
        (self.width / 8) as usize
    }
}

/// Describes an enum.
#[derive(Clone)]
pub struct Enum {
    /// Name of the enum.
    pub name: String,

    /// All possible values the enum can be.
    pub options: Vec<Field>,

    /// Flags for the enum data type, itself.
    pub flags: Flags
}

impl SizeableObject for Enum {
    fn size(&self, _: &ParsedDefinitions) -> usize {
        size_of::<u16>()
    }
}

/// Describes a field
#[derive(Clone)]
pub struct Field {
    /// Name of the field, itself.
    pub name: String,

    /// Name of the field, itself, formatted for Rust enums.
    pub name_rust_enum: String,

    /// Name of the field, itself, formatted for Rust fields.
    pub name_rust_field: String,

    /// Flags for this specific field.
    pub flags: Flags,

    /// Value of the field.
    ///
    /// For a bitfield, this is the binary AND.
    ///
    /// For an enum, this is the actual full value of the enum.
    pub value: u32
}

/// A list of engines that support something.
#[derive(Clone, Debug, Default)]
pub enum SupportedEngines {
    /// This is supported by all engines.
    #[default]
    AllEngines,

    /// This is only supported by these engines.
    SomeEngines(Vec<String>)
}

impl SupportedEngines {
    /// Returns true if the engine is supported.
    pub fn supports_engine(&self, engine: &Engine) -> bool {
        match self {
            Self::AllEngines => true,
            Self::SomeEngines(engines) => engines.contains(&engine.name)
        }
    }
}

/// General fields. Some may be applicable to some objects, but not all.
#[derive(Default, Clone)]
pub struct Flags {
    /// This field is not readable from tag files
    pub cache_only: bool,

    /// This field is not present in cache files
    pub non_cached: bool,

    /// Hint to the editor it should be read-only by default
    pub uneditable_in_editor: bool,

    /// Hint to the editor it should not be displayed by default
    pub hidden_in_editor: bool,

    /// The field cannot be used; if it is set, it will be lost
    pub exclude: bool,

    /// Store in little endian in tag format
    pub little_endian_in_tags: bool,

    /// The value is subtracted by 1 when put into a cache file (and incremented by 1 if extracted).
    pub shifted_by_one: bool,

    /// The value must be set.
    pub non_null: bool,

    /// Supported engines for the field.
    ///
    /// If unsupported, this is treated as padding.
    pub supported_engines: SupportedEngines,

    /// Any comment, if present
    pub comment: Option<String>,

    /// Any developer note, if present
    pub developer_note: Option<String>,

    /// Any description, if present
    pub description: Option<String>
}

impl Flags {
    pub(crate) fn combine_with(&mut self, other: &Flags) {
        self.cache_only |= other.cache_only;
        self.non_cached |= other.non_cached;
        self.uneditable_in_editor |= other.uneditable_in_editor;
        self.hidden_in_editor |= other.hidden_in_editor;
        self.exclude |= other.exclude;
        self.little_endian_in_tags |= other.little_endian_in_tags;
        self.shifted_by_one |= other.shifted_by_one;
        self.non_null |= other.non_null;
    }
}

/// Describes how to parse a cache file.
///
/// Note: This enum will be removed eventually to generify cache file loading/building.
#[derive(Copy, Clone, PartialEq)]
pub enum EngineCacheParser {
    /// Hint this is an Xbox cache file.
    Xbox,

    /// Hint this is a PC cache file.
    PC
}

/// Describes an engine.
pub struct Engine {
    /// Internal name of the engine.
    pub name: String,

    /// Displayed name of the engine.
    pub display_name: String,

    /// Full version of the engine.
    pub version: Option<String>,

    /// Short version of the engine (typically inserted into cache files).
    pub build: Option<Build>,

    /// Engine this engine inherits fields off of.
    pub inherits: Option<String>,

    /// If true, then this exact engine has actual cache files.
    pub build_target: bool,

    /// Used as a fallback if the engine cannot be precisely determined.
    ///
    /// NOTE: This property is set explicitly per engine.
    pub fallback: bool,

    /// If true, this refers to a custom, modded engine rather than an official release.
    pub custom: bool,

    /// Cache file version.
    pub cache_file_version: u32,

    /// This is the default engine for a given cache file version.
    ///
    /// NOTE: This property is set explicitly per engine.
    pub cache_default: bool,

    /// BSP data can be loaded externally from the actual BSP tag.
    pub external_bsps: bool,

    /// Model data is not located in tag data but in a model block in the cache file.
    pub external_models: bool,

    /// Maximum number of script nodes in the scenario tag.
    pub max_script_nodes: u64,

    /// Maximum tag space, in bytes.
    pub max_tag_space: u64,

    /// If true, models are lossily compressed.
    pub compressed_models: bool,

    /// Data alignment in bytes.
    pub data_alignment: u64,

    /// The cache file uses an obfuscated header layout.
    pub obfuscated_header_layout: bool,

    /// Describes how to read bitmaps in cache files.
    pub bitmap_options: EngineBitmapOptions,

    /// If `Some`, the engine uses external resource maps.
    pub resource_maps: Option<EngineSupportedResourceMaps>,

    /// Describes how to parse some parts of the cache file.
    pub cache_parser: EngineCacheParser,

    /// Maximum cache file size.
    pub max_cache_file_size: EngineCacheFileSize,

    /// Base memory address in tag data.
    pub base_memory_address: BaseMemoryAddress,

    /// List of all required tags to build a cache file (besides the scenario tag).
    pub required_tags: EngineRequiredTags,

    /// Type of compression.
    pub compression_type: EngineCompressionType,
}

/// Describes the type of compression used, if any.
pub enum EngineCompressionType {
    /// Cache files are stored uncompressed.
    Uncompressed,

    /// Uses DEFLATE (e.g. zlib) compression.
    Deflate
}

/// Describes additional fields.
///
/// Note: This will be changed to an enum, later.
pub struct EngineSupportedResourceMaps {
    /// Supports externally indexed tags.
    pub externally_indexed_tags: bool
}

/// Per-scenario type cache file size limits.
pub struct EngineCacheFileSize {
    /// Maximum cache file size, in bytes, for UI maps.
    pub user_interface: u64,

    /// Maximum cache file size, in bytes, for campaign maps.
    pub singleplayer: u64,

    /// Maximum cache file size, in bytes, for multiplayer maps.
    pub multiplayer: u64
}

/// All prerequisite tags for building a cache file.
#[derive(Default)]
pub struct EngineRequiredTags {
    /// All prerequisite tags for any maps.
    pub all: Vec<String>,

    /// All prerequisite tags for UI maps (in addition to `all`).
    pub user_interface: Vec<String>,

    /// All prerequisite tags for campaign maps (in addition to `all`).
    pub singleplayer: Vec<String>,

    /// All prerequisite tags for multiplayer maps (in addition to `all`).
    pub multiplayer: Vec<String>
}

/// Base memory address for the tag data block.
pub struct BaseMemoryAddress {
    /// The base memory address.
    pub address: u64,

    /// The base memory address is inferred from the tag data address, assuming it is always located
    /// directly after the tag data header.
    ///
    /// If so, `address` is a default value, but it is not required.
    pub inferred: bool
}

/// Describes the build string.
pub struct Build {
    /// The actual build string.
    ///
    /// Note: This is fewer than 32 characters and is inserted directly into a `String32`.
    pub string: String,

    /// Additional build strings that this engine can be used for.
    pub aliases: Vec<String>,

    /// If true, the build string is enforced and cannot differ or else the map will be rejected by
    /// the game.
    pub enforced: bool
}

/// Describes how bitmaps work on the engine.
///
/// This only applies to cache files. Tag files are unaffected.
pub struct EngineBitmapOptions {
    /// If true, uncompressed power-of-two bitmaps are swizzled.
    pub swizzled: bool,

    /// If true, the texture dimensions have to modulo block size.
    pub texture_dimension_must_modulo_block_size: bool,

    /// If true, cubemap faces on the same mipmap are not stored contiguously.
    pub cubemap_faces_stored_separately: bool,

    /// The bytes to align data to.
    pub alignment: u64
}

/// Describes a type of objects for a field.
#[derive(Clone)]
pub enum FieldObject {
    /// Describes an inline object.
    NamedObject(String),

    /// Describes a resizeable array of objects.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// use std::ffi::c_void;
    ///
    /// struct Reflexive<T> {
    ///     /// Number of elements.
    ///     count: u32,
    ///
    ///     /// Meaningless in tag files.
    ///     first_element: *mut T,
    ///
    ///     /// In tag and cache files, this is meaningless.
    ///     ///
    ///     /// However, in the official tools, this is used internally to store a pointer to
    ///     /// definitions.
    ///     tag_definitions: *mut c_void
    /// }
    /// ```
    Reflexive(String),

    /// Describes a reference to a tag.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// use std::ffi::c_char;
    ///
    /// type TagID = u32;
    ///
    /// struct TagReference {
    ///     /// Tag group fourcc
    ///     tag_group: u32,
    ///
    ///     /// Tag path (meaningless in tag files).
    ///     tag_path: *const c_char,
    ///
    ///     /// Length of the tag file (unused in cache files).
    ///     tag_path_length: u32,
    ///
    ///     /// Tag ID (in tag files, this is typically u32::MAX)
    ///     tag_id: TagID
    /// }
    TagReference {
        /// All allowed groups that can be referenced by this tag reference.
        allowed_groups: Vec<String>
    },

    /// Describes a tag group.
    ///
    /// This is represented by the tag group's fourcc.
    TagGroup,

    /// Describes a reference to unstructured tag data.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// use std::ffi::c_void;
    ///
    /// struct Data {
    ///     /// Length in bytes.
    ///     size: u32,
    ///
    ///     /// Unused in this case.
    ///     _unused_flags: u32,
    ///
    ///     /// Unused in this case.
    ///     _unused_file_offset: u32,
    ///
    ///     /// Pointer to the data in RAM (meaningless in tag files).
    ///     data: *mut c_void,
    ///
    ///     /// In tag and cache files, this is meaningless.
    ///     ///
    ///     /// However, in the official tools, this is used internally to store a pointer to
    ///     /// definitions.
    ///     tag_definitions: *mut c_void
    /// }
    /// ```
    Data,

    /// Describes BSP vertex data.
    ///
    /// Structure-wise, this is the same as a `Data`, but you can use this for specialized handling.
    BSPVertexData,

    /// Describes a null-terminated UTF-16 string.
    ///
    /// Structure-wise, this is the same as a `Data`, but you can use this for specialized handling.
    UTF16String,

    /// Describes data that is stored in the file rather than in RAM.
    ///
    /// This has the same structure as `Data`, but different fields are used.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// use std::ffi::c_void;
    ///
    /// struct FileData {
    ///     /// Length in bytes.
    ///     size: u32,
    ///
    ///     /// Flags (used for determining if the data is in a resource file)
    ///     flags: u32,
    ///
    ///     /// Offset to the data in the cache file.
    ///     file_offset: u32,
    ///
    ///     /// Pointer to the data in RAM (meaningless in tag files, unused in cache files).
    ///     data: *mut c_void,
    ///
    ///     /// In tag and cache files, this is meaningless.
    ///     ///
    ///     /// However, in the official tools, this is used internally to store a pointer to
    ///     /// definitions.
    ///     tag_definitions: *mut c_void
    /// }
    /// ```
    FileData,

    /// Describes a 32-bit float.
    F32,

    /// Describes an 8-bit unsigned integer.
    U8,

    /// Describes a 16-bit unsigned integer.
    U16,

    /// Describes a 32-bit unsigned integer.
    U32,

    /// Describes an 8-bit signed integer.
    I8,

    /// Describes a 16-bit signed integer.
    I16,

    /// Describes a 32-bit signed integer.
    I32,

    /// Describes a loose tag ID, stored as a 32-bit unsigned integer.
    TagID,

    /// Describes a loose table ID, stored as a 32-bit unsigned integer.
    ID,

    /// Describes an index of some kind, stored as a 16-bit integer.
    ///
    /// 0xFFFF means "Null" in this case.
    Index,

    /// Describes an angle, stored as a 32-bit float.
    ///
    /// You can use this to display things as degrees instead of radians.
    Angle,

    /// Describes a loose pointer.
    Address,

    /// Describes a two-dimensional vector.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Vector2D {
    ///     x: f32,
    ///     y: f32
    /// }
    /// ```
    Vector2D,

    /// Describes a three-dimensional vector.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Vector2D {
    ///     x: f32,
    ///     y: f32,
    ///     z: f32
    /// }
    /// ```
    Vector3D,

    /// Describes a 2D vector compressed into a 32-bit value.
    CompressedVector2D,

    /// Describes a 3D vector compressed into a 32-bit value.
    CompressedVector3D,

    /// Describes a float \[-1,1\] compressed into a 16-bit value.
    CompressedFloat,

    /// Describes a two-dimensional vector.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Vector2D {
    ///     x: i16,
    ///     y: i16
    /// }
    /// ```
    Vector2DInt,

    /// Describes a two-dimensional plane.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Plane2D {
    ///     offset: f32,
    ///     vector: Vector2D
    /// }
    ///
    /// struct Vector2D { x: f32, y: f32 }
    /// ```
    Plane2D,

    /// Describes a three-dimensional plane.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Plane3D {
    ///     offset: f32,
    ///     vector: Vector3D
    /// }
    ///
    /// struct Vector3D { x: f32, y: f32, z: f32 }
    /// ```
    Plane3D,

    /// Describes an Euler angle
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Euler2D {
    ///     yaw: f32,
    ///     pitch: f32
    /// }
    /// ```
    Euler2D,

    /// Describes an Euler angle
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Euler2D {
    ///     yaw: f32,
    ///     pitch: f32,
    ///     roll: f32
    /// }
    /// ```
    Euler3D,

    /// Describes a rectangle.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Rectangle {
    ///     top: i16,
    ///     left: i16,
    ///     bottom: i16,
    ///     right: i16
    /// }
    /// ```
    Rectangle,

    /// Describes a quaternion (4 floats).
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Quaternion {
    ///     x: f32,
    ///     y: f32,
    ///     z: f32,
    ///     w: f32,
    /// }
    /// ```
    Quaternion,

    /// Describes a 2x3 matrix (6 floats).
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Matrix2x3 {
    ///     forward: Vector3D,
    ///     up: Vector3D
    /// }
    ///
    /// struct Vector3D { x: f32, y: f32, z: f32 }
    /// ```
    Matrix2x3,

    /// Describes a 3x3 matrix (9 floats).
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct Matrix3x3 {
    ///     forward: Vector3D,
    ///     left: Vector3D,
    ///     up: Vector3D
    /// }
    ///
    /// struct Vector3D { x: f32, y: f32, z: f32 }
    /// ```
    Matrix3x3,

    /// Describes color without alpha.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct ColorRGB {
    ///     r: f32,
    ///     g: f32,
    ///     b: f32,
    /// }
    /// ```
    ColorRGB,

    /// Describes a color with alpha.
    ///
    /// Can be represented like this:
    ///
    /// ```
    /// struct ColorARGB {
    ///     a: f32,
    ///     rgb: ColorRGB
    /// }
    /// struct ColorRGB { r: f32, g: f32, b: f32 }
    /// ```
    ColorARGB,

    /// Describes an A8R8G8B8 color packed into an int.
    ///
    /// This is represented as `0xAARRGGBB`.
    Pixel32,

    /// Describes a null-terminated 31 character string.
    String32,

    /// Describes a value that can be stored in 32 bits.
    ///
    /// This is equivalent to a union.
    ScenarioScriptNodeValue,
}

impl FieldObject {
    const fn primitive_size(&self) -> usize {
        match self {
            Self::Reflexive(_) => 0xC,
            Self::TagReference { .. } => 0x10,
            Self::Data | Self::FileData | Self::BSPVertexData | Self::UTF16String => 0x14,
            Self::F32
            | Self::Angle
            | Self::U32
            | Self::Address
            | Self::I32
            | Self::Pixel32
            | Self::ID
            | Self::TagID
            | Self::CompressedVector2D
            | Self::CompressedVector3D => 0x4,
            Self::U16 | Self::I16 | Self::Index | Self::CompressedFloat => 0x2,
            Self::U8 | Self::I8 => 0x1,
            Self::Rectangle | Self::Vector2DInt => Self::I16.primitive_size() * self.composite_count(),
            Self::ScenarioScriptNodeValue => 0x4,
            Self::TagGroup => 0x4,
            Self::Vector2D
            | Self::Vector3D
            | Self::Plane2D
            | Self::Plane3D
            | Self::Quaternion
            | Self::Matrix2x3
            | Self::Matrix3x3
            | Self::ColorRGB
            | Self::Euler2D
            | Self::Euler3D
            | Self::ColorARGB => FieldObject::F32.primitive_size() * self.composite_count(),
            Self::String32 => 32,

            Self::NamedObject(_) => unreachable!()
        }
    }

    const fn composite_count(&self) -> usize {
        match self {
            Self::Reflexive(_) => 1,
            Self::TagReference { .. } => 1,
            Self::NamedObject(_) => 1,
            Self::Data | Self::FileData | Self::BSPVertexData | Self::UTF16String => 1,
            Self::TagID | Self::ID => 1,
            Self::TagGroup => 1,
            Self::F32 | Self::Angle | Self::U32 | Self::Address | Self::I32 | Self::Pixel32 | Self::CompressedVector2D | Self::CompressedVector3D | Self::CompressedFloat => 1,
            Self::U16 | Self::I16 | Self::Index => 1,
            Self::U8 | Self::I8 => 1,
            Self::Rectangle => 4,
            Self::Vector2D => 2,
            Self::Vector3D => 3,
            Self::Euler2D => 2,
            Self::Euler3D => 3,
            Self::Plane2D => 3,
            Self::Plane3D => 4,
            Self::Quaternion => 4,
            Self::Vector2DInt => 2,
            Self::Matrix2x3 => 2 * 3,
            Self::Matrix3x3 => 3 * 3,
            Self::ColorRGB => 3,
            Self::ColorARGB => 4,
            Self::String32 => 1,
            Self::ScenarioScriptNodeValue => 1,
        }
    }

    const fn primitive_value_type(&self) -> Option<StaticValue> {
        match self {
            Self::NamedObject(_)
            | Self::Data
            | Self::FileData
            | Self::BSPVertexData
            | Self::UTF16String
            | Self::TagID
            | Self::ID
            | Self::Address
            | Self::ScenarioScriptNodeValue
            | Self::TagGroup
            | Self::CompressedVector2D
            | Self::CompressedVector3D
            | Self::CompressedFloat => None,

            Self::TagReference { .. }
            | Self::String32 => Some(StaticValue::String(String::new())),

            Self::U8
            | Self::U16
            | Self::Index
            | Self::U32
            | Self::Pixel32
            | Self::Reflexive(_) => Some(StaticValue::Uint(0)),

            Self::I8
            | Self::I16
            | Self::I32
            | Self::Rectangle
            | Self::Vector2DInt => Some(StaticValue::Int(0)),

            Self::F32
            | Self::Angle
            | Self::Vector2D
            | Self::Vector3D
            | Self::Plane2D
            | Self::Plane3D
            | Self::Euler2D
            | Self::Euler3D
            | Self::Quaternion
            | Self::Matrix2x3
            | Self::Matrix3x3
            | Self::ColorRGB
            | Self::ColorARGB => Some(StaticValue::Float(0.0)),
        }
    }

    const fn is_const(&self) -> Option<bool> {
        Some(match self {
            FieldObject::NamedObject(_) => return None,
            FieldObject::Reflexive(_) => false,
            FieldObject::TagReference { .. } => false,
            FieldObject::TagGroup => true,
            FieldObject::Data => false,
            FieldObject::BSPVertexData => false,
            FieldObject::UTF16String => false,
            FieldObject::FileData => false,
            FieldObject::F32 => true,
            FieldObject::U8 => true,
            FieldObject::U16 => true,
            FieldObject::U32 => true,
            FieldObject::I8 => true,
            FieldObject::I16 => true,
            FieldObject::I32 => true,
            FieldObject::TagID => true,
            FieldObject::ID => true,
            FieldObject::Index => true,
            FieldObject::Angle => true,
            FieldObject::Address => true,
            FieldObject::Vector2D => true,
            FieldObject::Vector3D => true,
            FieldObject::CompressedVector2D => true,
            FieldObject::CompressedVector3D => true,
            FieldObject::CompressedFloat => true,
            FieldObject::Vector2DInt => true,
            FieldObject::Plane2D => true,
            FieldObject::Plane3D => true,
            FieldObject::Euler2D => true,
            FieldObject::Euler3D => true,
            FieldObject::Rectangle => true,
            FieldObject::Quaternion => true,
            FieldObject::Matrix2x3 => true,
            FieldObject::Matrix3x3 => true,
            FieldObject::ColorRGB => true,
            FieldObject::ColorARGB => true,
            FieldObject::Pixel32 => true,
            FieldObject::String32 => true,
            FieldObject::ScenarioScriptNodeValue => true,
        })
    }
}

impl SizeableObject for FieldObject {
    fn size(&self, parsed_tag_data: &ParsedDefinitions) -> usize {
        match self {
            Self::NamedObject(p) => parsed_tag_data.objects.get(p).unwrap().size(parsed_tag_data),
            _ => self.primitive_size()
        }
    }
}

mod parse;
pub(crate) use parse::*;
