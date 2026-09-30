//! Raw OpenGL (via miniquad) renderer for the voxel world. Chunk meshes live in
//! static GPU buffers; entities, sky and effects are rebuilt every frame into
//! one streaming buffer.

use crate::mesher::{ChunkMesh, MeshData, Vertex};
use crate::texture::{mip_levels, tile_uv, ATLAS};
use macroquad::math::{Mat4, Vec3, Vec4};
use macroquad::miniquad::*;
use std::collections::HashMap;

const VERTEX_SHADER: &str = r#"#version 100
attribute vec3 in_pos;
attribute vec2 in_uv;
attribute vec3 in_light;
attribute vec2 in_tile;

uniform mat4 mvp;
uniform vec4 params3;  // x: time, y: waving leaves, z: water reflections
uniform vec4 wave;     // where leaf tiles start in the atlas: oak (xy), spruce (zw)
uniform vec4 wave2;    // jungle leaves (xy), water (zw)

varying vec2 v_uv;
varying vec3 v_light;
varying vec3 v_wpos;
varying vec2 v_tile;

bool starts_at(vec2 t, vec2 o) {
    return abs(t.x - o.x) < 0.0001 && abs(t.y - o.y) < 0.0001;
}

void main() {
    vec3 p = in_pos;
    // Leaves sway a little in the wind. Neighbouring leaves share corners, which
    // move together, so the canopy bends without opening gaps.
    if (params3.y > 0.5 && in_tile.x >= 0.0 && (starts_at(in_tile, wave.xy) || starts_at(in_tile, wave.zw) || starts_at(in_tile, wave2.xy))) {
        float t = params3.x;
        p.x += sin(t * 1.6 + p.x * 0.6 + p.y * 0.4 + p.z * 0.2) * 0.04;
        p.z += cos(t * 1.3 + p.z * 0.6 + p.y * 0.3 + p.x * 0.2) * 0.04;
    }
    gl_Position = mvp * vec4(p, 1.0);
    v_uv = in_uv;
    v_light = in_light;
    v_wpos = p;
    v_tile = in_tile;
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 100
#extension GL_OES_standard_derivatives : enable
#extension GL_EXT_shader_texture_lod : enable
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

varying vec2 v_uv;
varying vec3 v_light;
varying vec3 v_wpos;
varying vec2 v_tile;

uniform sampler2D tex;
uniform vec4 cam_pos;
uniform vec4 fog_color;
uniform vec4 params;   // x: daylight, y: fog start, z: fog end, w: alpha multiplier
uniform vec4 params2;  // x: fullbright, y: least light anywhere (the Scorchlands glow), z: colour-blind view
uniform vec4 params3;  // x: time, y: waving leaves, z: water reflections
uniform vec4 wave2;    // zw: where the water tile starts in the atlas
DALTONIZE
uniform vec4 tint;
uniform vec4 lights[16];

// One tile's width in the atlas, and a hair to stay inside it.
const float TILE = TILE_SIZE;
const float EDGE = 1.0 / 4096.0;

vec4 sample_tile() {
    if (v_tile.x < 0.0) {
        // A face with one tile carries its corner as -corner - 2. With
        // multisampling, pixels on a shape's edge are shaded from their centre,
        // which can lie just outside the shape: keep that sample inside the tile
        // so no line of the next tile over shows along block edges.
        if (v_tile.x < -1.5) {
            vec2 o = -v_tile - 2.0;
            return texture2D(tex, clamp(v_uv, o + EDGE, o + TILE - EDGE));
        }
        return texture2D(tex, v_uv);
    }
    // A merged face repeats its tile. Mipmap choice follows the unwrapped
    // coordinates where the driver lets us, so the tile edges don't sparkle.
    vec2 uv = v_tile + EDGE + fract(v_uv) * (TILE - 2.0 * EDGE);
#if defined(GL_OES_standard_derivatives) && defined(GL_EXT_shader_texture_lod)
    return texture2DGradEXT(tex, uv, dFdx(v_uv) * TILE, dFdy(v_uv) * TILE);
#else
    return texture2D(tex, uv);
#endif
}

void main() {
#ifdef GL_OES_standard_derivatives
    // Which way the surface faces, worked out before any pixel is discarded.
    vec3 surface = cross(dFdx(v_wpos), dFdy(v_wpos));
#endif
    vec4 c = sample_tile() * tint;
    if (c.a < 0.08) discard;
    vec3 col;
    if (params2.x > 0.5) {
        col = c.rgb;
    } else if (v_light.x > 1.5) {
        // Glowing (lava): its own light, whatever the time of day.
        col = c.rgb * (v_light.x - 1.5);
    } else {
        float sky = v_light.y * params.x;
        // The world carries its own block light (z); things that move use the
        // nearby point lights. Lights marked moving (negative radius, like a
        // held torch) shine on everything.
        float bl = max(v_light.z, 0.0);
        for (int i = 0; i < 16; i++) {
            vec4 L = lights[i];
            if (L.w != 0.0 && (v_light.z < 0.0 || L.w < 0.0)) {
                float d = distance(v_wpos, L.xyz);
                bl = max(bl, clamp(1.0 - d / abs(L.w), 0.0, 1.0));
            }
        }
        float lvl = max(max(sky, bl), max(0.02, params2.y));
        // Torchlight is warm, daylight is neutral.
        vec3 warm = mix(vec3(1.0), vec3(1.0, 0.85, 0.6), clamp(bl - sky, 0.0, 1.0));
        col = c.rgb * v_light.x * lvl * warm;
    }
#ifdef GL_OES_standard_derivatives
    // Water reflects the sky: more of it at a glancing angle, and the sun glints off ripples.
    vec2 rel = v_uv - wave2.zw;
    bool water = v_tile.x < 0.0 && rel.x >= 0.0 && rel.y >= 0.0 && rel.x <= TILE && rel.y <= TILE;
    if (params3.z > 0.5 && water && params2.x < 0.5) {
        if (abs(surface.y) > 0.7 * length(surface)) {
            vec3 view = normalize(v_wpos - cam_pos.xyz);
            float fresnel = pow(1.0 - abs(view.y), 3.0);
            col = mix(col, fog_color.rgb * (0.55 + 0.45 * params.x), clamp(0.12 + fresnel * 0.7, 0.0, 0.8));
            float t = params3.x;
            float ripple = sin(v_wpos.x * 3.1 + t * 1.7 + sin(v_wpos.z * 1.3)) * sin(v_wpos.z * 2.7 - t * 1.3 + sin(v_wpos.x * 1.1));
            col += vec3(pow(max(ripple, 0.0), 20.0) * 0.6 * params.x);
        }
    }
#endif
    if (params.z > 0.0) {
        float d = distance(v_wpos, cam_pos.xyz);
        float f = clamp((d - params.y) / (params.z - params.y), 0.0, 1.0);
        col = mix(col, fog_color.rgb, f);
    }
    // Colour-blind friendly view (see access.rs).
    if (params2.z > 0.5) {
        col = daltonize(col);
    }
    gl_FragColor = vec4(col, c.a * params.w);
}
"#;

/// The world shaders rewritten for GLSL 1.50 (OpenGL 3.2 and later), with
/// every varying sampled at the centroid of the covered samples.
fn centroid_glsl(vertex: &str, fragment: &str) -> (String, String) {
    let vertex = vertex.replace("#version 100", "#version 150").replace("attribute ", "in ").replace("varying ", "centroid out ");
    let body: String = fragment.lines().filter(|l| !l.starts_with("#version") && !l.starts_with("#extension")).map(|l| format!("{l}\n")).collect();
    let body = body
        .replace("GL_OES_standard_derivatives", "HAS_DERIVATIVES")
        .replace("GL_EXT_shader_texture_lod", "HAS_TEXTURE_LOD")
        .replace("varying ", "centroid in ")
        .replace("texture2DGradEXT(", "textureGrad(")
        .replace("texture2D(", "texture(")
        .replace("gl_FragColor", "frag_color");
    let fragment = format!("#version 150\n#define HAS_DERIVATIVES 1\n#define HAS_TEXTURE_LOD 1\nout vec4 frag_color;\n{body}");
    (vertex, fragment)
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Uniforms {
    pub mvp: Mat4,
    pub cam_pos: Vec4,
    pub fog_color: Vec4,
    pub params: Vec4,
    pub params2: Vec4,
    pub tint: Vec4,
    pub lights: [Vec4; 16],
    pub params3: Vec4,
    pub wave: Vec4,
    pub wave2: Vec4,
}

fn shader_meta() -> ShaderMeta {
    ShaderMeta {
        images: vec!["tex".to_string()],
        uniforms: UniformBlockLayout {
            uniforms: vec![
                UniformDesc::new("mvp", UniformType::Mat4),
                UniformDesc::new("cam_pos", UniformType::Float4),
                UniformDesc::new("fog_color", UniformType::Float4),
                UniformDesc::new("params", UniformType::Float4),
                UniformDesc::new("params2", UniformType::Float4),
                UniformDesc::new("tint", UniformType::Float4),
                UniformDesc::new("lights", UniformType::Float4).array(16),
                UniformDesc::new("params3", UniformType::Float4),
                UniformDesc::new("wave", UniformType::Float4),
                UniformDesc::new("wave2", UniformType::Float4),
            ],
        },
    }
}

struct GpuMesh {
    vb: BufferId,
    ib: BufferId,
    count: i32,
}

pub struct GpuChunk {
    opaque: Option<GpuMesh>,
    water: Option<GpuMesh>,
    pub lights: Vec<[f32; 4]>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Pass {
    Opaque,
    Blend,
    /// Drawn over everything (first-person hand).
    Overlay,
    /// Sun, moon, stars: behind everything.
    Sky,
}

pub struct Batch {
    pub pass: Pass,
    pub start: usize,
    pub count: usize,
    pub tint: [f32; 4],
    pub fullbright: bool,
}

/// Per-frame immediate-mode geometry.
#[derive(Default)]
pub struct DynGeo {
    pub mesh: MeshData,
    pub batches: Vec<Batch>,
}

impl DynGeo {
    pub fn begin(&mut self, pass: Pass, tint: [f32; 4], fullbright: bool) {
        let start = self.mesh.idx.len();
        self.batches.push(Batch { pass, start, count: 0, tint, fullbright });
    }
    fn end_batch(&mut self) {
        let n = self.mesh.idx.len();
        if let Some(b) = self.batches.last_mut() {
            b.count = n - b.start;
        }
    }
    /// Quad from four world-space corners with a tile.
    pub fn quad(&mut self, c: [Vec3; 4], tile: u16, uv_rect: [f32; 4], light: [f32; 2]) {
        let (u0, v0, s) = tile_uv(tile);
        let (a, b, cc, d) = (uv_rect[0], uv_rect[1], uv_rect[2], uv_rect[3]);
        let uvs = [[a, d], [cc, d], [cc, b], [a, b]];
        let mut v = [Vertex::default(); 4];
        for i in 0..4 {
            v[i] = Vertex { pos: c[i].to_array(), uv: [u0 + uvs[i][0] * s, v0 + uvs[i][1] * s], light: [light[0], light[1], -1.0], tile: [-u0 - 2.0, -v0 - 2.0] };
        }
        self.mesh.quad(v, false);
        self.end_batch();
    }
    /// Oriented box: `m` maps the unit cube [0,1]^3 into world space.
    /// `tiles` order: +x, -x, +y, -y, +z, -z.
    pub fn cube(&mut self, m: &Mat4, tiles: [u16; 6], sky: f32, uv_rect: [f32; 4]) {
        for (f, (_, corners, shade)) in crate::mesher::FACES.iter().enumerate() {
            let c = corners.map(|p| m.transform_point3(Vec3::from_array(p)));
            self.quad(c, tiles[f], uv_rect, [*shade, sky]);
        }
    }
}

pub struct FrameParams {
    pub view_proj: Mat4,
    pub cam_pos: Vec3,
    pub fog_color: [f32; 3],
    pub fog_start: f32,
    pub fog_end: f32,
    pub daylight: f32,
    /// Least light anywhere (0 for the ordinary world).
    pub ambient: f32,
    pub lights: [Vec4; 16],
    /// Show the world through the colour-blind filter (see access.rs).
    pub colour_blind: bool,
    /// Graphics options: leaves sway, water reflects the sky; and the clock for both.
    pub waving_leaves: bool,
    pub water_reflections: bool,
    pub time: f32,
}

pub struct Renderer {
    opaque: Pipeline,
    blend: Pipeline,
    overlay: Pipeline,
    sky: Pipeline,
    pub texture: TextureId,
    dyn_vb: BufferId,
    dyn_ib: BufferId,
    dyn_cap_v: usize,
    dyn_cap_i: usize,
    pub chunks: HashMap<(i32, i32), GpuChunk>,
}

fn upload(ctx: &mut dyn RenderingBackend, m: &MeshData) -> Option<GpuMesh> {
    if m.idx.is_empty() {
        return None;
    }
    let vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Immutable, BufferSource::slice(&m.verts));
    let ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Immutable, BufferSource::slice(&m.idx));
    Some(GpuMesh { vb, ib, count: m.idx.len() as i32 })
}

fn free(ctx: &mut dyn RenderingBackend, m: Option<GpuMesh>) {
    if let Some(m) = m {
        ctx.delete_buffer(m.vb);
        ctx.delete_buffer(m.ib);
    }
}

impl Renderer {
    pub fn new(ctx: &mut dyn RenderingBackend, atlas: &[u8]) -> Self {
        let texture = ctx.new_texture(
            TextureAccess::Static,
            TextureSource::Bytes(atlas),
            TextureParams {
                kind: TextureKind::Texture2D,
                format: TextureFormat::RGBA8,
                wrap: TextureWrap::Clamp,
                min_filter: FilterMode::Nearest,
                mag_filter: FilterMode::Nearest,
                mipmap_filter: MipmapFilterMode::Nearest,
                width: ATLAS as u32,
                height: ATLAS as u32,
                allocate_mipmaps: true,
                sample_count: 1,
            },
        );
        upload_mips(ctx, texture, atlas);

        let (_, _, tile) = tile_uv(0);
        let fragment = FRAGMENT_SHADER.replace("TILE_SIZE", &format!("{tile:.8}")).replace("DALTONIZE", crate::access::DALTONIZE_GLSL);
        // Newer OpenGL can shade the pixels on a shape's edge from inside the
        // shape (centroid sampling), so thin faces seen edge-on don't pick up
        // light or texture from beyond their corners under multisampling.
        let glsl = ctx.info().glsl_support;
        let modern = (glsl.v150 || glsl.v330).then(|| centroid_glsl(VERTEX_SHADER, &fragment));
        let shader = modern
            .and_then(|(v, f)| ctx.new_shader(ShaderSource::Glsl { vertex: &v, fragment: &f }, shader_meta()).ok())
            .map(Ok)
            .unwrap_or_else(|| ctx.new_shader(ShaderSource::Glsl { vertex: VERTEX_SHADER, fragment: &fragment }, shader_meta()))
            .unwrap_or_else(|e| panic!("shader failed to compile: {e:?}"));
        let layout = [BufferLayout::default()];
        let attrs = [
            VertexAttribute::new("in_pos", VertexFormat::Float3),
            VertexAttribute::new("in_uv", VertexFormat::Float2),
            VertexAttribute::new("in_light", VertexFormat::Float3),
            VertexAttribute::new("in_tile", VertexFormat::Float2),
        ];
        let alpha = Some(BlendState::new(
            Equation::Add,
            BlendFactor::Value(BlendValue::SourceAlpha),
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
        ));
        let mut pipe = |params: PipelineParams| ctx.new_pipeline(&layout, &attrs, shader, params);
        let opaque = pipe(PipelineParams {
            cull_face: CullFace::Back,
            depth_test: Comparison::LessOrEqual,
            depth_write: true,
            ..Default::default()
        });
        let blend = pipe(PipelineParams {
            cull_face: CullFace::Nothing,
            depth_test: Comparison::LessOrEqual,
            // miniquad disables the depth *test* whenever depth_write is false, which let
            // water, clouds and outlines show through terrain. Keep the test on here and
            // mask depth writes by hand around the translucent pass instead.
            depth_write: true,
            color_blend: alpha,
            alpha_blend: alpha,
            ..Default::default()
        });
        let overlay = pipe(PipelineParams {
            cull_face: CullFace::Back,
            depth_test: Comparison::Always,
            depth_write: false,
            color_blend: alpha,
            ..Default::default()
        });
        let sky = pipe(PipelineParams {
            cull_face: CullFace::Nothing,
            depth_test: Comparison::Always,
            depth_write: false,
            color_blend: alpha,
            ..Default::default()
        });

        let (dyn_cap_v, dyn_cap_i) = (1 << 16, 3 << 15);
        let dyn_vb = ctx.new_buffer(BufferType::VertexBuffer, BufferUsage::Stream, BufferSource::empty::<Vertex>(dyn_cap_v));
        let dyn_ib = ctx.new_buffer(BufferType::IndexBuffer, BufferUsage::Stream, BufferSource::empty::<u32>(dyn_cap_i));
        Renderer { opaque, blend, overlay, sky, texture, dyn_vb, dyn_ib, dyn_cap_v, dyn_cap_i, chunks: HashMap::new() }
    }

    pub fn set_chunk(&mut self, ctx: &mut dyn RenderingBackend, key: (i32, i32), mesh: ChunkMesh) {
        if let Some(old) = self.chunks.remove(&key) {
            free(ctx, old.opaque);
            free(ctx, old.water);
        }
        let g = GpuChunk { opaque: upload(ctx, &mesh.opaque), water: upload(ctx, &mesh.water), lights: mesh.lights };
        self.chunks.insert(key, g);
    }

    /// Replace the texture atlas (after mods change) and rebuild its mipmaps.
    pub fn update_atlas(&mut self, ctx: &mut dyn RenderingBackend, atlas: &[u8]) {
        ctx.texture_update(self.texture, atlas);
        upload_mips(ctx, self.texture, atlas);
    }

    pub fn drop_chunk(&mut self, ctx: &mut dyn RenderingBackend, key: (i32, i32)) {
        if let Some(old) = self.chunks.remove(&key) {
            free(ctx, old.opaque);
            free(ctx, old.water);
        }
    }

    pub fn clear(&mut self, ctx: &mut dyn RenderingBackend) {
        let keys: Vec<_> = self.chunks.keys().copied().collect();
        for k in keys {
            self.drop_chunk(ctx, k);
        }
    }

    /// Nearest light sources to the camera, packed for the shader.
    pub fn nearby_lights(&self, cam: Vec3, extra: &[[f32; 4]]) -> [Vec4; 16] {
        let (ccx, ccz) = ((cam.x / 16.0).floor() as i32, (cam.z / 16.0).floor() as i32);
        let mut all: Vec<(f32, [f32; 4])> = Vec::new();
        for dz in -2..=2 {
            for dx in -2..=2 {
                if let Some(c) = self.chunks.get(&(ccx + dx, ccz + dz)) {
                    for l in &c.lights {
                        let d = Vec3::new(l[0], l[1], l[2]).distance_squared(cam);
                        all.push((d, *l));
                    }
                }
            }
        }
        for l in extra {
            all.push((Vec3::new(l[0], l[1], l[2]).distance_squared(cam), *l));
        }
        all.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut out = [Vec4::ZERO; 16];
        for (i, (_, l)) in all.iter().take(16).enumerate() {
            out[i] = Vec4::from_array(*l);
        }
        out
    }

    pub fn draw(&mut self, ctx: &mut dyn RenderingBackend, fp: &FrameParams, geo: &DynGeo) {
        let planes = frustum_planes(&fp.view_proj);
        let base = Uniforms {
            mvp: fp.view_proj,
            cam_pos: fp.cam_pos.extend(1.0),
            fog_color: Vec4::new(fp.fog_color[0], fp.fog_color[1], fp.fog_color[2], 1.0),
            params: Vec4::new(fp.daylight, fp.fog_start, fp.fog_end, 1.0),
            params2: Vec4::new(0.0, fp.ambient, fp.colour_blind as u8 as f32, 0.0),
            tint: Vec4::ONE,
            lights: fp.lights,
            params3: Vec4::new(fp.time, fp.waving_leaves as u8 as f32, fp.water_reflections as u8 as f32, 0.0),
            wave: {
                let (a, b) = (tile_uv(crate::texture::T_LEAVES), tile_uv(crate::texture::T_SPRUCE_LEAVES));
                Vec4::new(a.0, a.1, b.0, b.1)
            },
            wave2: {
                let (a, b) = (tile_uv(crate::texture::T_JUNGLE_LEAVES), tile_uv(crate::texture::T_WATER));
                Vec4::new(a.0, a.1, b.0, b.1)
            },
        };

        // Upload streaming geometry (clamped to capacity).
        let nv = geo.mesh.verts.len().min(self.dyn_cap_v);
        let ni = geo.mesh.idx.len().min(self.dyn_cap_i);
        if nv > 0 && ni > 0 {
            ctx.buffer_update(self.dyn_vb, BufferSource::slice(&geo.mesh.verts[..nv]));
            ctx.buffer_update(self.dyn_ib, BufferSource::slice(&geo.mesh.idx[..ni]));
        }
        let dyn_bind = Bindings { vertex_buffers: vec![self.dyn_vb], index_buffer: self.dyn_ib, images: vec![self.texture] };
        let draw_batches = |ctx: &mut dyn RenderingBackend, pass: Pass, pipe: &Pipeline| {
            let mut applied = false;
            for b in geo.batches.iter().filter(|b| b.pass == pass && b.count > 0 && b.start + b.count <= ni) {
                if !applied {
                    ctx.apply_pipeline(pipe);
                    ctx.apply_bindings(&dyn_bind);
                    applied = true;
                }
                let mut u = base;
                u.tint = Vec4::from_array(b.tint);
                if b.fullbright || pass == Pass::Sky {
                    u.params2.x = 1.0;
                }
                if pass == Pass::Sky {
                    u.params.z = 0.0;
                }
                ctx.apply_uniforms(UniformsSource::table(&u));
                ctx.draw(b.start as i32, b.count as i32, 1);
            }
        };

        ctx.begin_default_pass(PassAction::Nothing);

        draw_batches(ctx, Pass::Sky, &self.sky);

        // Opaque chunks
        ctx.apply_pipeline(&self.opaque);
        ctx.apply_uniforms(UniformsSource::table(&base));
        let mut visible: Vec<((i32, i32), f32)> = Vec::new();
        for (&(cx, cz), c) in &self.chunks {
            let min = Vec3::new(cx as f32 * 16.0, 0.0, cz as f32 * 16.0);
            let max = min + Vec3::new(16.0, 128.0, 16.0);
            if !aabb_visible(&planes, min, max) {
                continue;
            }
            let center = min + Vec3::new(8.0, 0.0, 8.0);
            let d = Vec3::new(center.x - fp.cam_pos.x, 0.0, center.z - fp.cam_pos.z).length_squared();
            visible.push(((cx, cz), d));
            if let Some(m) = &c.opaque {
                ctx.apply_bindings(&Bindings { vertex_buffers: vec![m.vb], index_buffer: m.ib, images: vec![self.texture] });
                ctx.draw(0, m.count, 1);
            }
        }

        draw_batches(ctx, Pass::Opaque, &self.opaque);

        // Water, far to near
        visible.sort_by(|a, b| b.1.total_cmp(&a.1));
        ctx.apply_pipeline(&self.blend);
        set_depth_writes(false);
        ctx.apply_uniforms(UniformsSource::table(&base));
        for (k, _) in &visible {
            if let Some(m) = self.chunks.get(k).and_then(|c| c.water.as_ref()) {
                ctx.apply_bindings(&Bindings { vertex_buffers: vec![m.vb], index_buffer: m.ib, images: vec![self.texture] });
                ctx.draw(0, m.count, 1);
            }
        }

        draw_batches(ctx, Pass::Blend, &self.blend);
        // Restore before anything else (including macroquad's clears) touches depth.
        set_depth_writes(true);
        draw_batches(ctx, Pass::Overlay, &self.overlay);

        ctx.end_render_pass();
    }
}

/// Mipmaps built per tile (see `texture::mip_levels`). The driver fills the
/// levels below one texel a tile, then those are cut off where GL allows, so
/// distant faces never sample neighbouring tiles.
fn upload_mips(ctx: &mut dyn RenderingBackend, texture: TextureId, atlas: &[u8]) {
    crate::texture::remember_alpha(atlas);
    ctx.texture_generate_mipmaps(texture);
    let levels = mip_levels(atlas);
    #[allow(irrefutable_let_patterns)] // Metal is another variant on Apple targets
    let RawId::OpenGl(id) = (unsafe { ctx.texture_raw_id(texture) }) else { return };
    unsafe {
        gl::glBindTexture(gl::GL_TEXTURE_2D, id);
        gl::glPixelStorei(gl::GL_UNPACK_ALIGNMENT, 1);
        for (i, px) in levels.iter().enumerate() {
            let size = (ATLAS >> (i + 1)) as i32;
            gl::glTexSubImage2D(gl::GL_TEXTURE_2D, (i + 1) as i32, 0, 0, size, size, gl::GL_RGBA, gl::GL_UNSIGNED_BYTE, px.as_ptr() as *const _);
        }
        gl::glTexParameteri(gl::GL_TEXTURE_2D, gl::GL_TEXTURE_MAX_LEVEL, levels.len() as i32);
        // Older GL ES has no max level; the driver's deeper levels stay then.
        while gl::glGetError() != gl::GL_NO_ERROR {}
        gl::glBindTexture(gl::GL_TEXTURE_2D, 0);
    }
}

/// miniquad never changes the depth mask itself, so this sticks until restored.
fn set_depth_writes(on: bool) {
    unsafe { gl::glDepthMask(if on { gl::GL_TRUE } else { gl::GL_FALSE } as _) }
}

fn frustum_planes(m: &Mat4) -> [Vec4; 6] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    [r3 + r0, r3 - r0, r3 + r1, r3 - r1, r3 + r2, r3 - r2]
}

fn aabb_visible(planes: &[Vec4; 6], min: Vec3, max: Vec3) -> bool {
    planes.iter().all(|p| {
        let v = Vec3::new(
            if p.x >= 0.0 { max.x } else { min.x },
            if p.y >= 0.0 { max.y } else { min.y },
            if p.z >= 0.0 { max.z } else { min.z },
        );
        p.x * v.x + p.y * v.y + p.z * v.z + p.w >= 0.0
    })
}
