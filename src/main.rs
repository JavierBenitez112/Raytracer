use raylib::prelude::*;
use std::f32::consts::PI;

mod framebuffer;
mod ray_intersect;
mod cube;
mod camera;
mod light;
mod material;
mod textures;

use framebuffer::Framebuffer;
use ray_intersect::{Intersect, RayIntersect};
use cube::Cube;
use camera::Camera;
use light::Light;
use material::{Material, vector3_to_color};
use textures::TextureManager;

const ORIGIN_BIAS: f32 = 1e-4;
const SKYBOX_COLOR: Vector3 = Vector3::new(0.26, 0.55, 0.89);
const GRID_SIZE: usize = 10;
const CUBE_SIZE: f32 = 0.5;
const CUBE_SPACING: f32 = 0.6;

fn offset_origin(intersect: &Intersect, direction: &Vector3) -> Vector3 {
    let offset = intersect.normal * ORIGIN_BIAS;
    if direction.dot(intersect.normal) < 0.0 {
        intersect.point - offset
    } else {
        intersect.point + offset
    }
}

fn reflect(incident: &Vector3, normal: &Vector3) -> Vector3 {
    *incident - *normal * 2.0 * incident.dot(*normal)
}

fn refract(incident: &Vector3, normal: &Vector3, refractive_index: f32) -> Option<Vector3> {
    let mut cosi = incident.dot(*normal).max(-1.0).min(1.0);
    let mut etai = 1.0;
    let mut etat = refractive_index;
    let mut n = *normal;

    if cosi > 0.0 {
        std::mem::swap(&mut etai, &mut etat);
        n = -n;
    } else {
        cosi = -cosi;
    }

    let eta = etai / etat;
    let k = 1.0 - eta * eta * (1.0 - cosi * cosi);

    if k < 0.0 {
        None
    } else {
        Some(*incident * eta + n * (eta * cosi - k.sqrt()))
    }
}

fn cast_shadow(
    intersect: &Intersect,
    light: &Light,
    objects: &[Cube],
) -> f32 {
    let light_dir = (light.position - intersect.point).normalized();
    let light_distance = (light.position - intersect.point).length();

    let shadow_ray_origin = offset_origin(intersect, &light_dir);

    for object in objects {
        let shadow_intersect = object.ray_intersect(&shadow_ray_origin, &light_dir);
        if shadow_intersect.is_intersecting && shadow_intersect.distance < light_distance {
            return 1.0;
        }
    }

    0.0
}

pub fn cast_ray(
    ray_origin: &Vector3,
    ray_direction: &Vector3,
    objects: &[Cube],
    light: &Light,
    texture_manager: &TextureManager,
    depth: u32,
) -> Vector3 {
    if depth > 3 {
        return SKYBOX_COLOR;
    }

    let mut intersect = Intersect::empty();
    let mut zbuffer = f32::INFINITY;

    for object in objects {
        let i = object.ray_intersect(ray_origin, ray_direction);
        if i.is_intersecting && i.distance < zbuffer {
            zbuffer = i.distance;
            intersect = i;
        }
    }

    if !intersect.is_intersecting {
        return SKYBOX_COLOR;
    }

    let light_dir = (light.position - intersect.point).normalized();
    let view_dir = (*ray_origin - intersect.point).normalized();

    let mut normal = intersect.normal;
    if let Some(normal_map_path) = &intersect.material.normal_map_id {
        let texture = texture_manager.get_texture(normal_map_path).unwrap();
        let width = texture.width() as u32;
        let height = texture.height() as u32;
        let tx = (intersect.u * width as f32) as u32;
        let ty = (intersect.v * height as f32) as u32;

        if let Some(tex_normal) = texture_manager.get_normal_from_map(normal_map_path, tx, ty) {
            let tangent = Vector3::new(normal.y, -normal.x, 0.0).normalized();
            let bitangent = normal.cross(tangent);
            
            let transformed_normal_x = tex_normal.x * tangent.x + tex_normal.y * bitangent.x + tex_normal.z * normal.x;
            let transformed_normal_y = tex_normal.x * tangent.y + tex_normal.y * bitangent.y + tex_normal.z * normal.y;
            let transformed_normal_z = tex_normal.x * tangent.z + tex_normal.y * bitangent.z + tex_normal.z * normal.z;

            normal = Vector3::new(transformed_normal_x, transformed_normal_y, transformed_normal_z).normalized();
        }
    }

    let reflect_dir = reflect(&-light_dir, &normal).normalized();

    let shadow_intensity = cast_shadow(&intersect, light, objects);
    let light_intensity = light.intensity * (1.0 - shadow_intensity);

    let diffuse_color = if let Some(texture_path) = &intersect.material.texture_id {
        let texture = texture_manager.get_texture(texture_path).unwrap();
        let width = texture.width() as u32;
        let height = texture.height() as u32;
        let tx = (intersect.u * width as f32) as u32;
        let ty = (intersect.v * height as f32) as u32;
        let color = texture_manager.get_pixel_color(texture_path, tx, ty);
        color
    } else {
        intersect.material.diffuse
    };

    let diffuse_intensity = normal.dot(light_dir).max(0.0) * light_intensity;
    let diffuse = diffuse_color * diffuse_intensity;

    let specular_intensity = view_dir.dot(reflect_dir).max(0.0).powf(intersect.material.specular) * light_intensity;
    let light_color_v3 = Vector3::new(light.color.r as f32 / 255.0, light.color.g as f32 / 255.0, light.color.b as f32 / 255.0);
    let specular = light_color_v3 * specular_intensity;

    let albedo = intersect.material.albedo;
    let phong_color = diffuse * albedo[0] + specular * albedo[1];

    let reflectivity = intersect.material.albedo[2];
    let reflect_color = if reflectivity > 0.0 {
        let reflect_dir = reflect(ray_direction, &normal).normalized();
        let reflect_origin = offset_origin(&intersect, &reflect_dir);
        cast_ray(&reflect_origin, &reflect_dir, objects, light, texture_manager, depth + 1)
    } else {
        Vector3::zero()
    };

    let transparency = intersect.material.albedo[3];
    let refract_color = if transparency > 0.0 {
        if let Some(refract_dir) = refract(ray_direction, &normal, intersect.material.refractive_index) {
            let refract_origin = offset_origin(&intersect, &refract_dir);
            cast_ray(&refract_origin, &refract_dir, objects, light, texture_manager, depth + 1)
        } else {
            let reflect_dir = reflect(ray_direction, &normal).normalized();
            let reflect_origin = offset_origin(&intersect, &reflect_dir);
            cast_ray(&reflect_origin, &reflect_dir, objects, light, texture_manager, depth + 1)
        }
    } else {
        Vector3::zero()
    };

    phong_color * (1.0 - reflectivity - transparency) + reflect_color * reflectivity + refract_color * transparency
}

pub fn render(
    framebuffer: &mut Framebuffer,
    objects: &[Cube],
    camera: &Camera,
    light: &Light,
    texture_manager: &TextureManager,
) {
    let width = framebuffer.width as f32;
    let height = framebuffer.height as f32;
    let aspect_ratio = width / height;
    let fov = PI / 3.0;
    let perspective_scale = (fov * 0.5).tan();

    for y in 0..framebuffer.height {
        for x in 0..framebuffer.width {
            let screen_x = (2.0 * x as f32) / width - 1.0;
            let screen_y = -(2.0 * y as f32) / height + 1.0;

            let screen_x = screen_x * aspect_ratio * perspective_scale;
            let screen_y = screen_y * perspective_scale;

            let ray_direction = Vector3::new(screen_x, screen_y, -1.0).normalized();
            
            let rotated_direction = camera.basis_change(&ray_direction);

            let pixel_color_v3 = cast_ray(&camera.eye, &rotated_direction, objects, light, texture_manager, 0);
            let pixel_color = vector3_to_color(pixel_color_v3);

            framebuffer.set_current_color(pixel_color);
            framebuffer.set_pixel(x, y);
        }
    }
}

fn get_material_from_letter(letter: char) -> Option<Material> {
    match letter {
        'R' => Some(Material::new(
            Vector3::new(0.8, 0.2, 0.2),
            10.0,
            [0.9, 0.1, 0.0, 0.0],
            0.0,
            None,
            None,
        )),
        'B' => Some(Material::new(
            Vector3::new(0.8, 0.4, 0.2),
            20.0,
            [0.8, 0.2, 0.0, 0.0],
            0.0,
            None,
            None,
        )),
        'I' => Some(Material::new(
            Vector3::new(0.4, 0.4, 0.3),
            50.0,
            [0.6, 0.3, 0.1, 0.0],
            0.0,
            None,
            None,
        )),
        'G' => Some(Material::new(
            Vector3::new(0.6, 0.7, 0.8),
            125.0,
            [0.0, 0.5, 0.1, 0.8],
            1.5,
            None,
            None,
        )),
        'Y' => Some(Material::new(
            Vector3::new(0.9, 0.9, 0.2),
            30.0,
            [0.7, 0.3, 0.0, 0.0],
            0.0,
            None,
            None,
        )),
        'P' => Some(Material::new(
            Vector3::new(0.8, 0.2, 0.8),
            15.0,
            [0.8, 0.2, 0.0, 0.0],
            0.0,
            None,
            None,
        )),
        'C' => Some(Material::new(
            Vector3::new(0.2, 0.8, 0.8),
            25.0,
            [0.7, 0.3, 0.0, 0.0],
            0.0,
            None,
            None,
        )),
        'W' => Some(Material::new(
            Vector3::new(0.9, 0.9, 0.9),
            40.0,
            [0.6, 0.4, 0.0, 0.0],
            0.0,
            None,
            None,
        )),
        'K' => Some(Material::new(
            Vector3::new(0.1, 0.1, 0.1),
            5.0,
            [0.9, 0.1, 0.0, 0.0],
            0.0,
            None,
            None,
        )),
        _ => None,
    }
}

fn create_cube_from_letter(
    letter: char,
    grid_x: usize,
    grid_y: usize,
    layer: usize,
) -> Option<Cube> {
    if let Some(material) = get_material_from_letter(letter) {
        let offset = (GRID_SIZE as f32 - 1.0) * CUBE_SPACING / 2.0;
        let x = grid_x as f32 * CUBE_SPACING - offset;
        let y = layer as f32 * CUBE_SPACING;
        let z = grid_y as f32 * CUBE_SPACING - offset;
        
        Some(Cube {
            center: Vector3::new(x, y, z),
            size: CUBE_SIZE,
            material,
        })
    } else {
        None
    }
}

fn create_cubes_from_layers(layers: &[&[&str]]) -> Vec<Cube> {
    let mut cubes = Vec::new();
    
    for (layer_idx, layer) in layers.iter().enumerate() {
        for (y, line) in layer.iter().enumerate() {
            if y >= GRID_SIZE {
                break;
            }
            
            let chars: Vec<char> = line.chars().collect();
            for (x, &ch) in chars.iter().enumerate() {
                if x >= GRID_SIZE {
                    break;
                }
                
                if let Some(cube) = create_cube_from_letter(ch, x, y, layer_idx) {
                    cubes.push(cube);
                }
            }
        }
    }
    
    cubes
}

fn main() {
    let window_width = 1300;
    let window_height = 900;
 
    let (mut window, thread) = raylib::init()
        .size(window_width, window_height)
        .title("Raytracer Example")
        .log_level(TraceLogLevel::LOG_WARNING)
        .build();

    let mut texture_manager = TextureManager::new();
    // texture_manager.load_texture(&mut window, &thread, "assets/ball.png");
    // texture_manager.load_texture(&mut window, &thread, "assets/ball_normal.png");
    // texture_manager.load_texture(&mut window, &thread, "assets/bricks.png");
    // texture_manager.load_texture(&mut window, &thread, "assets/bricks_normal.png");
    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);

    let layers = [
        &[
            "          ",
            "          ",
            "  RRRRRR  ",
            "  R    R  ",
            "  R    R  ",
            "  R    R  ",
            "  RRRRRR  ",
            "          ",
            "          ",
            "          ",
        ][..],
        &[
            "          ",
            "   BBBB   ",
            "  B    B  ",
            "  B    B  ",
            "  BBBBBB  ",
            "  B    B  ",
            "  B    B  ",
            "   BBBB   ",
            "          ",
            "          ",
        ][..],
        &[
            "    II    ",
            "    II    ",
            "    II    ",
            "    II    ",
            "    II    ",
            "    II    ",
            "  IIIIII  ",
            "          ",
            "          ",
            "          ",
        ][..],
    ];

    let objects = create_cubes_from_layers(&layers);

    let mut camera = Camera::new(
        Vector3::new(0.0, 0.0, 5.0),
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
    );
    let rotation_speed = PI / 100.0;
    let zoom_speed = 0.1;

    let light = Light::new(
        Vector3::new(1.0, -1.0, 5.0),
        Color::new(255, 255, 255, 255),
        1.5,
    );

    while !window.window_should_close() {
        if window.is_key_down(KeyboardKey::KEY_LEFT) {
            camera.orbit(rotation_speed, 0.0);
        }
        if window.is_key_down(KeyboardKey::KEY_RIGHT) {
            camera.orbit(-rotation_speed, 0.0);
        }
        if window.is_key_down(KeyboardKey::KEY_UP) {
            camera.orbit(0.0, -rotation_speed);
        }
        if window.is_key_down(KeyboardKey::KEY_DOWN) {
            camera.orbit(0.0, rotation_speed);
        }
        if window.is_key_down(KeyboardKey::KEY_W) {
            camera.zoom(zoom_speed);
        }
        if window.is_key_down(KeyboardKey::KEY_S) {
            camera.zoom(-zoom_speed);
        }

        if camera.is_changed() {
            render(&mut framebuffer, &objects, &camera, &light, &texture_manager);
        }
        
        framebuffer.swap_buffers(&mut window, &thread);
    }
}
