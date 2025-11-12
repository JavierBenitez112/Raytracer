use raylib::prelude::Vector3;
use crate::ray_intersect::{Intersect, RayIntersect};
use crate::material::Material;

pub struct Cube {
    pub center: Vector3,
    pub size: f32,
    pub material: Material,
}

impl Cube {
    fn get_uv(&self, point: &Vector3, normal: &Vector3) -> (f32, f32) {
        let local = *point - self.center;
        let half_size = self.size / 2.0;
        
        // Determinar qué cara del cubo estamos mirando basándonos en la normal
        let abs_normal = Vector3::new(normal.x.abs(), normal.y.abs(), normal.z.abs());
        
        let (u, v) = if abs_normal.x > abs_normal.y && abs_normal.x > abs_normal.z {
            // Cara X (izquierda/derecha)
            ((local.z / half_size + 1.0) / 2.0, (local.y / half_size + 1.0) / 2.0)
        } else if abs_normal.y > abs_normal.z {
            // Cara Y (arriba/abajo)
            ((local.x / half_size + 1.0) / 2.0, (local.z / half_size + 1.0) / 2.0)
        } else {
            // Cara Z (frente/atrás)
            ((local.x / half_size + 1.0) / 2.0, (local.y / half_size + 1.0) / 2.0)
        };
        
        (u, v)
    }
}

impl RayIntersect for Cube {
    fn ray_intersect(&self, ray_origin: &Vector3, ray_direction: &Vector3) -> Intersect {
        let half_size = self.size / 2.0;
        let min = Vector3::new(
            self.center.x - half_size,
            self.center.y - half_size,
            self.center.z - half_size,
        );
        let max = Vector3::new(
            self.center.x + half_size,
            self.center.y + half_size,
            self.center.z + half_size,
        );

        // Algoritmo de intersección ray-box (slab method)
        let inv_dir = Vector3::new(
            1.0 / ray_direction.x,
            1.0 / ray_direction.y,
            1.0 / ray_direction.z,
        );

        let t1 = (min.x - ray_origin.x) * inv_dir.x;
        let t2 = (max.x - ray_origin.x) * inv_dir.x;
        let t3 = (min.y - ray_origin.y) * inv_dir.y;
        let t4 = (max.y - ray_origin.y) * inv_dir.y;
        let t5 = (min.z - ray_origin.z) * inv_dir.z;
        let t6 = (max.z - ray_origin.z) * inv_dir.z;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        // Si tmax < 0, el cubo está detrás del rayo
        if tmax < 0.0 {
            return Intersect::empty();
        }

        // Si tmin > tmax, el rayo no intersecta el cubo
        if tmin > tmax {
            return Intersect::empty();
        }

        // Usar tmin si es positivo, de lo contrario tmax (estamos dentro del cubo)
        let t = if tmin > 0.0 { tmin } else { tmax };

        let point = *ray_origin + *ray_direction * t;

        // Calcular la normal basada en qué cara del cubo golpeamos
        let mut normal = Vector3::zero();
        let epsilon = 0.0001;
        
        if (point.x - min.x).abs() < epsilon {
            normal = Vector3::new(-1.0, 0.0, 0.0);
        } else if (point.x - max.x).abs() < epsilon {
            normal = Vector3::new(1.0, 0.0, 0.0);
        } else if (point.y - min.y).abs() < epsilon {
            normal = Vector3::new(0.0, -1.0, 0.0);
        } else if (point.y - max.y).abs() < epsilon {
            normal = Vector3::new(0.0, 1.0, 0.0);
        } else if (point.z - min.z).abs() < epsilon {
            normal = Vector3::new(0.0, 0.0, -1.0);
        } else if (point.z - max.z).abs() < epsilon {
            normal = Vector3::new(0.0, 0.0, 1.0);
        }

        let (u, v) = self.get_uv(&point, &normal);

        Intersect::new(point, normal, t, self.material.clone(), u, v)
    }
}

