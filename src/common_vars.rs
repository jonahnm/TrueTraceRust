pub struct vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
impl vec3 {
    const ZERO: Self = vec3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
    pub fn distance(&self,other: &Self) -> f32 {
        f32::sqrt((other.x - self.x).powi(2) + (other.y - self.y).powi(2) + (other.z - self.z).powi(2))
    }
}
pub struct vec2 {
    pub x: f32,
    pub y: f32,
}
impl vec2 {
    const ZERO: Self = vec2 {
        x: 0.0,
        y: 0.0,
    };

    pub fn distance(&self,other: &Self) -> f32 {
        f32::sqrt((other.x - self.x).powi(2) + (other.y - self.y).powi(2))
    }
}
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
pub struct BoundingSphere {
    pub center: vec3,
    pub radius: f32,
}
impl BoundingSphere {
    pub fn init() -> BoundingSphere {
        BoundingSphere {
            center: vec3::ZERO,
            radius: 0.0,
        }
    }
    pub fn validate(&mut self,padding: f32) {
        self.radius = f32::max(self.radius, padding);
    }

    pub fn extend(&mut self,a: vec3) {
        self.radius = f32::max(self.radius, a.distance(&self.center));
    }
}

pub struct LightBVHTransform {
    pub transform: nalgebra::Matrix4<f32>,
    pub solid_offset: i32,
}

pub struct GaussianTreeNode {
    pub s: BoundingSphere,
    pub axis: vec3,
    pub variance: f32,
    pub sharpness: f32,
    pub intensity: f32,
    pub left: i32,
}
pub struct LightData {
    pub radiance: vec3,
    pub position: vec3,
    pub direction: vec3,
    pub Type: i32,
    pub spot_angle: vec2,
    pub zaxis_rotation: f32,
    pub softness: f32,
    pub ies_tex: (i32,i32),
}
pub struct LightMapTriData {
    pub pos0: vec3,
    pub posedge1: vec3,
    pub posedge2: vec3,
    pub lmuv0: vec2,
    pub lmuv1: vec2,
    pub lmuv2: vec2,
    pub norm1: u32,
    pub norm2: u32,
    pub norm3: u32,
}

pub struct LightMapData {
    pub light_map_index: i32,
    pub light_map_tris: Vec<LightMapTriData>,
}

pub struct MeshDat {
    pub cur_vertex_offset: i32,
    pub vertices: Vec<vec3>,
    pub normals: Vec<vec3>,
    pub tangents: Vec<(f32,f32,f32,f32)>,
    pub uvs: Vec<vec2>,
    pub colors: Vec<Color>,
    pub mat_dat: Vec<i32>,
    pub indices: Vec<i32>,
}
impl MeshDat {
    pub fn init(starting_size: usize) -> MeshDat {
        MeshDat {
            uvs: vec![vec2::ZERO;starting_size],
            vertices: vec![vec3::ZERO;starting_size],
            normals: vec![vec3::ZERO;starting_size],
            tangents: vec![(0.0,0.0,0.0,0.0);starting_size],
            colors: vec![Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 0.0,
            };starting_size],
            indices: vec![0;starting_size],
            mat_dat: vec![0;starting_size / 3],
            cur_vertex_offset: 0,
        }
    }
    pub fn clear(&mut self) {
        self.uvs.clear();
        self.colors.clear();
        self.indices.clear();
        self.mat_dat.clear();
        self.vertices.clear();
        self.normals.clear();
        self.tangents.clear();
    }
}

pub struct PerInstanceData {
    pub object_to_world: nalgebra::Matrix4<f32>,
    pub rendering_layer_mask: u32,
    pub custom_instance_id: u32,
}

pub struct IntersectionMatData {
    pub alpha_tex: (i32,i32),
    pub albedo_tex: (i32,i32),
    pub tag: i32,
    pub mat_type: i32,
    pub spec_trans: f32,
    pub alpha_cutoff: f32,
    pub albedo_tex_scale: (f32,f32,f32,f32),
    pub surface_color: vec3,
    pub rotation: f32,
    pub scatter_distance: f32,
}

pub struct MatTextureData {
    
}

