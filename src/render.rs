//! Raw OpenGL (via miniquad) renderer for the voxel world. Chunk meshes live in
//! static GPU buffers; entities, sky and effects are rebuilt every frame into
//! one streaming buffer.

use crate::mesher::{ChunkMesh, MeshData, Vertex};
use crate::texture::{tile_uv, ATLAS};
use macroquad::math::{Mat4, Vec3, Vec4};
use macroquad::miniquad::*;
use std::collections::HashMap;

const VERTEX_SHADER: &str = r#"#version 100
attribute vec3 in_pos;
attribute vec2 in_uv;
attribute vec2 in_light;

uniform mat4 mvp;

varying vec2 v_uv;
varying vec2 v_light;
varying vec3 v_wpos;

void main() {
    gl_Position = mvp * vec4(in_pos, 1.0);
    v_uv = in_uv;
    v_light = in_light;
    v_wpos = in_pos;
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 100
#ifdef GL_FRAGMENT_PRECISION_HIGH
precision highp float;
#else
precision mediump float;
#endif

varying vec2 v_uv;
varying vec2 v_light;
varying vec3 v_wpos;

uniform sampler2D tex;
uniform vec4 cam_pos;
uniform vec4 fog_color;
uniform vec4 params;   // x: daylight, y: fog start, z: fog end, w: alpha multiplier
uniform vec4 params2;  // x: fullbright
uniform vec4 tint;
uniform vec4 lights[16];

void main() {
    vec4 c = texture2D(tex, v_uv) * tint;
    if (c.a < 0.08) discard;
    vec3 col;
    if (params2.x > 0.5) {
        col = c.rgb;
    } else {
        float sky = v_light.y * params.x;
        float bl = 0.0;
        for (int i = 0; i < 16; i++) {
            vec4 L = lights[i];
            if (L.w > 0.0) {
                float d = distance(v_wpos, L.xyz);
                bl = max(bl, clamp(1.0 - d / L.w, 0.0, 1.0));
            }
        }
        float lvl = max(max(sky, bl), 0.05);
        // Torchlight is warm, daylight is neutral.
        vec3 warm = mix(vec3(1.0), vec3(1.0, 0.85, 0.6), clamp(bl - sky, 0.0, 1.0));
        col = c.rgb * v_light.x * lvl * warm;
    }
    if (params.z > 0.0) {
        float d = distance(v_wpos, cam_pos.xyz);
        float f = clamp((d - params.y) / (params.z - params.y), 0.0, 1.0);
        col = mix(col, fog_color.rgb, f);
    }
    gl_FragColor = vec4(col, c.a * params.w);
}
"#;

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
            v[i] = Vertex { pos: c[i].to_array(), uv: [u0 + uvs[i][0] * s, v0 + uvs[i][1] * s], light };
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
    pub lights: [Vec4; 16],
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
        ctx.texture_generate_mipmaps(texture);

        let shader = ctx
            .new_shader(ShaderSource::Glsl { vertex: VERTEX_SHADER, fragment: FRAGMENT_SHADER }, shader_meta())
            .unwrap_or_else(|e| panic!("shader failed to compile: {e:?}"));
        let layout = [BufferLayout::default()];
        let attrs = [
            VertexAttribute::new("in_pos", VertexFormat::Float3),
            VertexAttribute::new("in_uv", VertexFormat::Float2),
            VertexAttribute::new("in_light", VertexFormat::Float2),
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
        ctx.texture_generate_mipmaps(self.texture);
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
            params2: Vec4::ZERO,
            tint: Vec4::ONE,
            lights: fp.lights,
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
