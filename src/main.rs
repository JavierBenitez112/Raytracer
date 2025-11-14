use raylib::prelude::*;
use std::f32::consts::PI;
use rayon::prelude::*;

mod framebuffer;
mod ray_intersect;
mod cube;
mod camera;
mod light;
mod material;
mod textures;
mod blocks;

use framebuffer::Framebuffer;
use ray_intersect::{Intersect, RayIntersect};
use cube::Cube;
use camera::Camera;
use light::Light;
use material::vector3_to_color;
use textures::TextureManager;
use blocks::{create_cubes_from_layers, get_layers};

const ORIGIN_BIAS: f32 = 1e-4;
const SKYBOX_COLOR: Vector3 = Vector3::new(0.26, 0.55, 0.89);

// Función para rotar un vector alrededor del eje Y
fn rotate_around_y(point: Vector3, angle: f32) -> Vector3 {
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    Vector3::new(
        point.x * cos_a - point.z * sin_a,
        point.y,
        point.x * sin_a + point.z * cos_a,
    )
}

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
        // Ignorar bloques emisivos (glowstone) al calcular sombras
        if object.material.is_emissive {
            continue;
        }
        
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
        let texture_color = texture_manager.get_pixel_color(texture_path, tx, ty);
        let texture_alpha = texture_manager.get_pixel_alpha(texture_path, tx, ty);
        
        // Si el píxel es transparente, mezclar con el color difuso del material
        // Para materiales transparentes como vidrio, esto permite que la refracción se vea mejor
        intersect.material.diffuse * (1.0 - texture_alpha) + texture_color * texture_alpha
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

    // Calcular iluminación de bloques emisivos (glowstone)
    let mut emissive_light = Vector3::zero();
    for object in objects {
        if object.material.is_emissive {
            let emissive_dir = (object.center - intersect.point).normalized();
            let emissive_distance = (object.center - intersect.point).length();
            
            // Solo considerar bloques emisivos cercanos (dentro de un radio razonable)
            if emissive_distance < 10.0 && emissive_distance > 0.01 {
                // Verificar si hay sombra entre el punto y el bloque emisivo
                let mut blocked = false;
                let emissive_ray_origin = offset_origin(&intersect, &emissive_dir);
                
                for other_object in objects {
                    // Ignorar el propio objeto emisivo y otros emisivos
                    if other_object.material.is_emissive {
                        continue;
                    }
                    
                    let shadow_check = other_object.ray_intersect(&emissive_ray_origin, &emissive_dir);
                    if shadow_check.is_intersecting && shadow_check.distance < emissive_distance {
                        blocked = true;
                        break;
                    }
                }
                
                if !blocked {
                    // Calcular contribución de luz basada en distancia (atenuación)
                    let attenuation = 1.0 / (1.0 + 0.1 * emissive_distance * emissive_distance);
                    let emissive_intensity = normal.dot(emissive_dir).max(0.0) * object.material.emission_intensity * attenuation;
                    // Multiplicar por el color de la textura del objeto iluminado para que se vea la textura
                    emissive_light += object.material.emission_color * emissive_intensity * diffuse_color;
                }
            }
        }
    }

    // Agregar emisión propia si el objeto es emisivo
    // La emisión se modifica por la textura si está disponible
    let self_emission = if intersect.material.is_emissive {
        let emission_base = intersect.material.emission_color * intersect.material.emission_intensity;
        
        // Si hay textura, multiplicar la emisión por el color de la textura para que sea visible
        if let Some(texture_path) = &intersect.material.texture_id {
            let texture = texture_manager.get_texture(texture_path).unwrap();
            let width = texture.width() as u32;
            let height = texture.height() as u32;
            let tx = (intersect.u * width as f32) as u32;
            let ty = (intersect.v * height as f32) as u32;
            let texture_color = texture_manager.get_pixel_color(texture_path, tx, ty);
            // Combinar la emisión con la textura (la textura modula la emisión)
            emission_base * texture_color
        } else {
            emission_base
        }
    } else {
        Vector3::zero()
    };

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

    phong_color * (1.0 - reflectivity - transparency) + reflect_color * reflectivity + refract_color * transparency + emissive_light + self_emission
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

    // Crear un buffer temporal para almacenar los colores de los píxeles
    let mut pixel_buffer: Vec<Color> = vec![Color::BLACK; (framebuffer.width * framebuffer.height) as usize];

    // Paralelizar el renderizado por filas
    pixel_buffer.par_chunks_mut(framebuffer.width as usize).enumerate().for_each(|(y, row)| {
        for (x, pixel) in row.iter_mut().enumerate() {
            let screen_x = (2.0 * x as f32) / width - 1.0;
            let screen_y = -(2.0 * y as f32) / height + 1.0;

            let screen_x = screen_x * aspect_ratio * perspective_scale;
            let screen_y = screen_y * perspective_scale;

            let ray_direction = Vector3::new(screen_x, screen_y, -1.0).normalized();
            
            let rotated_direction = camera.basis_change(&ray_direction);

            let pixel_color_v3 = cast_ray(&camera.eye, &rotated_direction, objects, light, texture_manager, 0);
            *pixel = vector3_to_color(pixel_color_v3);
        }
    });

    // Copiar el buffer temporal al framebuffer
    for y in 0..framebuffer.height {
        for x in 0..framebuffer.width {
            let index = (y * framebuffer.width + x) as usize;
            framebuffer.set_pixel_color(x, y, pixel_buffer[index]);
        }
    }
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
    // Cargar todas las texturas de assets
    texture_manager.load_texture(&mut window, &thread, "assets/Furnace.png");
    texture_manager.load_texture(&mut window, &thread, "assets/Bookshelf.png");
    texture_manager.load_texture(&mut window, &thread, "assets/obsidiana.png");
    texture_manager.load_texture(&mut window, &thread, "assets/glass.png");
    texture_manager.load_texture(&mut window, &thread, "assets/glowstone.png");
    texture_manager.load_texture(&mut window, &thread, "assets/chest.png");
    texture_manager.load_texture(&mut window, &thread, "assets/wood_planks.png");
    texture_manager.load_texture(&mut window, &thread, "assets/oak-wood-planks.png");
    texture_manager.load_texture(&mut window, &thread, "assets/ball_normal.png");
    let mut framebuffer = Framebuffer::new(window_width as u32, window_height as u32);

    let layers = get_layers();
    let base_objects = create_cubes_from_layers(layers);

    let mut camera = Camera::new(
        Vector3::new(0.0, 0.0, 5.0),
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(0.0, 1.0, 0.0),
    );
    let rotation_speed = PI / 100.0;
    let zoom_speed = 0.15;
    let diorama_rotation_speed = PI / 80.0;
    let mut diorama_angle = 0.0;

    // Configuración del ciclo día/noche (luz rotando alrededor del eje Y como el sol)
    let sun_radius = 8.0; // Radio de la órbita del sol
    let mut sun_angle = 0.0; // Ángulo inicial (0 = mediodía)
    let sun_rotation_speed = PI / 300.0; // Velocidad del ciclo día/noche
    
    let mut light = Light::new(
        Vector3::new(0.0, sun_radius, 0.0),
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
        
        // Rotación del diorama con Q y E
        if window.is_key_down(KeyboardKey::KEY_Q) {
            diorama_angle += diorama_rotation_speed;
        }
        if window.is_key_down(KeyboardKey::KEY_E) {
            diorama_angle -= diorama_rotation_speed;
        }
        
        // Rotar todos los objetos del diorama alrededor del eje Y
        let rotated_objects: Vec<Cube> = base_objects.iter().map(|cube| {
            let rotated_center = rotate_around_y(cube.center, diorama_angle);
            Cube {
                center: rotated_center,
                size: cube.size,
                material: cube.material.clone(),
            }
        }).collect();

        // Ciclo día/noche: rotar el sol alrededor del eje Y
        sun_angle += sun_rotation_speed;
        
        // Calcular posición del sol (rotación en el plano XZ, altura en Y)
        // El sol se mueve en un arco: alto durante el día, bajo durante la noche
        // sun_angle: 0 = mediodía (alto), PI/2 = atardecer, PI = medianoche (bajo), 3*PI/2 = amanecer
        let sun_height = sun_angle.cos(); // 1 (mediodía) a -1 (medianoche)
        // Rotación horizontal alrededor del eje Y
        let sun_x = sun_radius * sun_angle.cos();
        let sun_y = sun_radius * sun_height; // Altura del sol
        let sun_z = sun_radius * sun_angle.sin();
        
        light.position = Vector3::new(sun_x, sun_y, sun_z);
        
        // Calcular intensidad de la luz según la altura del sol
        // Durante el día (sun_height > 0): más intensa
        // Durante la noche (sun_height < 0): menos intensa
        let normalized_height = (sun_height + 1.0) / 2.0; // Normalizar de 0 a 1
        light.intensity = 0.1 + normalized_height * 1.4; // De 0.1 (noche) a 1.5 (día)
        
        // Calcular color de la luz según la hora del día
        // Amanecer/Atardecer: cálido (naranja/rojo)
        // Día: blanco/azul claro
        // Noche: azul oscuro/morado
        let (r, g, b) = if normalized_height > 0.7 {
            // Día (alto en el cielo)
            (255, 255, 255)
        } else if normalized_height > 0.3 {
            // Amanecer/Atardecer
            let warmth = (normalized_height - 0.3) / 0.4; // 0 a 1
            let r_val = (255.0 * (1.0 - warmth * 0.3) + 255.0 * warmth) as u8;
            let g_val = (200.0 * (1.0 - warmth * 0.2) + 255.0 * warmth) as u8;
            let b_val = (150.0 * (1.0 - warmth * 0.5) + 255.0 * warmth) as u8;
            (r_val, g_val, b_val)
        } else {
            // Noche
            let night_factor = normalized_height / 0.3; // 0 a 1
            let r_val = (100.0 * night_factor) as u8;
            let g_val = (120.0 * night_factor) as u8;
            let b_val = (180.0 * night_factor) as u8;
            (r_val, g_val, b_val)
        };
        
        light.color = Color::new(r, g, b, 255);

        // Renderizar siempre ya que la luz está rotando continuamente
        render(&mut framebuffer, &rotated_objects, &camera, &light, &texture_manager);
        
        framebuffer.swap_buffers(&mut window, &thread);
    }
}
