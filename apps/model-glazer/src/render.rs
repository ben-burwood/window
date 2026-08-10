//! The OpenGL renderer: a single shader program that draws the loaded mesh
//! inside an egui paint callback.
//!
//! egui is a 2D toolkit, so 3D is drawn through eframe's `glow` backend: we get
//! the shared [`glow::Context`] from the paint callback and issue raw GL. The
//! shader derives a per-face normal from screen-space derivatives of the view
//! position, giving correct flat shading for any mesh without us storing or
//! reconstructing normals. A wireframe mode swaps to `GL_LINE` polygons and a
//! flat colour.

use egui_glow::glow::{self, HasContext};

const VERT_SRC: &str = r#"#version 330 core
layout (location = 0) in vec3 in_pos;
uniform mat4 u_mvp;
uniform mat4 u_mv;
out vec3 v_view;
void main() {
    v_view = (u_mv * vec4(in_pos, 1.0)).xyz;
    gl_Position = u_mvp * vec4(in_pos, 1.0);
}
"#;

const FRAG_SRC: &str = r#"#version 330 core
in vec3 v_view;
uniform vec3 u_color;
uniform int u_flat;   // 1 = emit u_color unshaded (wireframe)
out vec4 frag_color;
void main() {
    if (u_flat == 1) {
        frag_color = vec4(u_color, 1.0);
        return;
    }
    // Flat face normal from the derivatives of the view-space position; a
    // headlight at the camera means brightness follows how square-on the face
    // is (|n.z| in view space).
    vec3 n = normalize(cross(dFdx(v_view), dFdy(v_view)));
    float d = abs(n.z);
    vec3 c = u_color * (0.25 + 0.75 * d);
    frag_color = vec4(c, 1.0);
}
"#;

/// The current shading mode chosen in the toolbar.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shading {
    Solid,
    Wireframe,
}

/// A rectangle of the framebuffer to render into, in physical pixels (as given
/// by egui's `PaintCallbackInfo::viewport_in_pixels`).
pub struct Viewport {
    pub left: i32,
    pub bottom: i32,
    pub width: i32,
    pub height: i32,
}

pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    ebo: glow::Buffer,
    index_count: i32,
    u_mvp: Option<glow::UniformLocation>,
    u_mv: Option<glow::UniformLocation>,
    u_color: Option<glow::UniformLocation>,
    u_flat: Option<glow::UniformLocation>,
}

impl Renderer {
    /// Compile the program and allocate (empty) buffers. Returns an error string
    /// if shader compilation or linking fails.
    pub fn new(gl: &glow::Context) -> Result<Renderer, String> {
        unsafe {
            let program = link_program(gl, VERT_SRC, FRAG_SRC)?;
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            let ebo = gl.create_buffer()?;

            let u_mvp = gl.get_uniform_location(program, "u_mvp");
            let u_mv = gl.get_uniform_location(program, "u_mv");
            let u_color = gl.get_uniform_location(program, "u_color");
            let u_flat = gl.get_uniform_location(program, "u_flat");

            Ok(Renderer {
                program,
                vao,
                vbo,
                ebo,
                index_count: 0,
                u_mvp,
                u_mv,
                u_color,
                u_flat,
            })
        }
    }

    /// (Re)upload a mesh's vertex and index buffers. Called on each file load.
    pub fn upload(&mut self, gl: &glow::Context, mesh: &crate::mesh::Mesh) {
        unsafe {
            gl.bind_vertex_array(Some(self.vao));

            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                as_bytes(&mesh.positions),
                glow::STATIC_DRAW,
            );

            gl.bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(self.ebo));
            gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                as_bytes(&mesh.indices),
                glow::STATIC_DRAW,
            );

            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, 0, 0);

            gl.bind_vertex_array(None);
            self.index_count = mesh.indices.len() as i32;
        }
    }

    /// Draw the uploaded mesh into `viewport` with the given transforms.
    ///
    /// `mvp` and `mv` are column-major 4×4 matrices (glam's `to_cols_array`).
    pub fn paint(
        &self,
        gl: &glow::Context,
        mvp: &[f32; 16],
        mv: &[f32; 16],
        shading: Shading,
        viewport: Viewport,
    ) {
        if self.index_count == 0 {
            return;
        }
        unsafe {
            // Confine to the canvas and give ourselves a fresh depth range;
            // egui doesn't use depth, so clearing it here is safe.
            gl.viewport(
                viewport.left,
                viewport.bottom,
                viewport.width,
                viewport.height,
            );
            gl.enable(glow::SCISSOR_TEST);
            gl.scissor(
                viewport.left,
                viewport.bottom,
                viewport.width,
                viewport.height,
            );
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LESS);
            gl.disable(glow::BLEND);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);

            gl.use_program(Some(self.program));
            gl.uniform_matrix_4_f32_slice(self.u_mvp.as_ref(), false, mvp);
            gl.uniform_matrix_4_f32_slice(self.u_mv.as_ref(), false, mv);

            match shading {
                Shading::Solid => {
                    gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
                    gl.uniform_1_i32(self.u_flat.as_ref(), 0);
                    gl.uniform_3_f32(self.u_color.as_ref(), 0.72, 0.74, 0.78);
                }
                Shading::Wireframe => {
                    gl.polygon_mode(glow::FRONT_AND_BACK, glow::LINE);
                    gl.uniform_1_i32(self.u_flat.as_ref(), 1);
                    gl.uniform_3_f32(self.u_color.as_ref(), 0.30, 0.72, 0.95);
                }
            }

            gl.bind_vertex_array(Some(self.vao));
            gl.draw_elements(glow::TRIANGLES, self.index_count, glow::UNSIGNED_INT, 0);
            gl.bind_vertex_array(None);

            // Restore state egui relies on for the rest of the frame.
            gl.polygon_mode(glow::FRONT_AND_BACK, glow::FILL);
            gl.disable(glow::DEPTH_TEST);
            gl.disable(glow::SCISSOR_TEST);
            gl.use_program(None);
        }
    }

    /// Free GL resources. Call from `eframe::App::on_exit`, where the context is
    /// still current.
    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
            gl.delete_buffer(self.ebo);
        }
    }
}

/// Compile + link a vertex/fragment program, returning the GL error log on failure.
unsafe fn link_program(
    gl: &glow::Context,
    vert: &str,
    frag: &str,
) -> Result<glow::Program, String> {
    let program = gl.create_program()?;
    let mut shaders = Vec::new();
    for (kind, src) in [(glow::VERTEX_SHADER, vert), (glow::FRAGMENT_SHADER, frag)] {
        let shader = gl.create_shader(kind)?;
        gl.shader_source(shader, src);
        gl.compile_shader(shader);
        if !gl.get_shader_compile_status(shader) {
            let log = gl.get_shader_info_log(shader);
            gl.delete_shader(shader);
            gl.delete_program(program);
            return Err(format!("shader compile failed: {log}"));
        }
        gl.attach_shader(program, shader);
        shaders.push(shader);
    }
    gl.link_program(program);
    if !gl.get_program_link_status(program) {
        let log = gl.get_program_info_log(program);
        return Err(format!("program link failed: {log}"));
    }
    for shader in shaders {
        gl.detach_shader(program, shader);
        gl.delete_shader(shader);
    }
    Ok(program)
}

/// Reinterpret a slice of plain-old-data (`f32`/`u32` here) as bytes for
/// `buffer_data_u8_slice`.
fn as_bytes<T: Copy>(v: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr() as *const u8, std::mem::size_of_val(v)) }
}
