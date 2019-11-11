
use tri_mesh::prelude::*;
use tri_mesh::prelude::Vec3 as Vec3;
use tri_mesh::prelude::vec3 as vec3;
use tri_mesh::prelude::vec4 as vec4;
use std::collections::HashMap;

/// Loads the mesh and scale/translate it.
fn on_startup(scene_center: &Vec3, scene_radius: f64) -> tri_mesh::mesh::Mesh
{
    let mut mesh = MeshBuilder::new().with_obj(include_str!("../assets/models/suzanne.obj").to_string()).build().unwrap();
    let (min, max) = mesh.extreme_coordinates();
    mesh.translate(-0.5 * (max + min)); // Translate such that the mesh center is in origo.
    let size = max - min;
    mesh.scale(0.5 * scene_radius / size.x.max(size.y).max(size.z)); // Scale the mesh such that the size of the biggest side of the bounding box is half a scene radius
    mesh.translate(*scene_center); // Translate the mesh to the scene center
    mesh
}

/// When the user clicks, we see if the model is hit. If it is, we compute the morph weights from the picking point.
fn on_click(mesh: &tri_mesh::mesh::Mesh, ray_start_point: &Vec3, ray_direction: &Vec3) -> Option<HashMap<VertexID, Vec3>>
{
    if let Some((vertex_id, point)) = pick(&mesh,&ray_start_point, &ray_direction) {
        Some(compute_weights(mesh, vertex_id, &point))
    }
    else {None}
}

/// Morphs the vertices based on the computed weights.
fn on_morph(mesh: &mut tri_mesh::mesh::Mesh, weights: &HashMap<VertexID, Vec3>, factor: f64)
{
    for (vertex_id, weight) in weights.iter() {
        mesh.move_vertex_by(*vertex_id,weight * factor);
    }
}

/// Picking used for determining whether a mouse click starts a morph operation. Returns a close vertex and the position of the click on the mesh surface.
fn pick(mesh: &tri_mesh::mesh::Mesh, ray_start_point: &Vec3, ray_direction: &Vec3) -> Option<(VertexID, Vec3)>
{
    if let Some(Intersection::Point {primitive, point}) = mesh.ray_intersection(ray_start_point, ray_direction) {
        let start_vertex_id = match primitive {
            Primitive::Face(face_id) => {
                mesh.walker_from_face(face_id).vertex_id().unwrap()
            },
            Primitive::Edge(halfedge_id) => {
                let (vertex_id, ..) = mesh.edge_vertices(halfedge_id);
                vertex_id
            },
            Primitive::Vertex(vertex_id) => {
                vertex_id
            }
        };
        Some((start_vertex_id, point))
    }
    else {None}
}

/// Compute a directional weight for each vertex to be used for the morph operation.
fn compute_weights(mesh: &tri_mesh::mesh::Mesh, start_vertex_id: VertexID, start_point: &Vec3) -> HashMap<VertexID, Vec3>
{
    static SQR_MAX_DISTANCE: f64 = 1.0;

    // Use the smoothstep function to get a smooth morphing
    let smoothstep_function = |sqr_distance| {
        let x = sqr_distance / SQR_MAX_DISTANCE;
        1.0 - x*x*(3.0 - 2.0 * x)
    };

    // Visit all the vertices close to the start vertex.
    let mut weights = HashMap::new();
    let mut to_be_tested = vec![start_vertex_id];
    while let Some(vertex_id) = to_be_tested.pop()
    {
        let sqr_distance = start_point.distance2(*mesh.vertex_position(vertex_id));
        if sqr_distance < SQR_MAX_DISTANCE
        {
            // The weight is computed as the smoothstep function to the square euclidean distance
            // to the start point on the surface multiplied by the vertex normal.
            weights.insert(vertex_id, smoothstep_function(sqr_distance) * mesh.vertex_normal(vertex_id));

            // Add neighbouring vertices to be tested if they have not been visited yet
            for halfedge_id in mesh.vertex_halfedge_iter(vertex_id)
            {
                let neighbour_vertex_id = mesh.walker_from_halfedge(halfedge_id).vertex_id().unwrap();
                if !weights.contains_key(&neighbour_vertex_id) {
                    to_be_tested.push(neighbour_vertex_id);
                }
            }
        }
    }
    weights
}

///
/// Above: Everything related to tri-mesh
/// Below: Visualisation of the mesh, event handling and so on
///
use dust::*;
use dust::window::{event::*, Window};

fn main()
{
    let args: Vec<String> = std::env::args().collect();
    let screenshot_path = if args.len() > 1 { Some(args[1].clone()) } else {None};

    let scene_radius = 10.0;
    let scene_center = dust::vec3(0.0, 5.0, 0.0);
    let mut mesh = on_startup(&vec3(scene_center.x as f64, scene_center.y as f64, scene_center.z as f64), scene_radius as f64);
    let positions: Vec<f32> = mesh.positions_buffer().iter().map(|v| *v as f32).collect();
    let normals: Vec<f32> = mesh.normals_buffer().iter().map(|v| *v as f32).collect();

    let mut window = Window::new_default("Morph tool").unwrap();
    let (width, height) = window.framebuffer_size();
    let window_size = window.size();
    let gl = window.gl();

    // Renderer
    let mut renderer = DeferredPipeline::new(&gl, width, height, vec4(0.8, 0.8, 0.8, 1.0)).unwrap();

    renderer.camera.set_view(scene_center + scene_radius * vec3(1.0, 1.0, 1.0).normalize(), scene_center,
                                                    vec3(0.0, 1.0, 0.0));

    // Objects
    let mut wireframe_model = ShadedEdges::new(&gl, &mesh.indices_buffer(), &positions, 0.01);
    wireframe_model.diffuse_intensity = 0.8;
    wireframe_model.specular_intensity = 0.2;
    wireframe_model.specular_power = 5.0;
    wireframe_model.color = vec3(0.9, 0.2, 0.2);

    let mut mesh_shader = MeshShader::new(&gl).unwrap();
    mesh_shader.color = vec3(0.8, 0.8, 0.8);
    mesh_shader.diffuse_intensity = 0.2;
    mesh_shader.specular_intensity = 0.4;
    mesh_shader.specular_power = 20.0;

    let mut model = dust::Mesh::new(&gl, &mesh.indices_buffer(), &positions, &normals).unwrap();
    let plane = dust::Mesh::new_plane(&gl).unwrap();

    renderer.ambient_light().set_intensity(0.4);

    let mut dir = vec3(-1.0, -1.0, -1.0).normalize();
    let mut light = renderer.spot_light(0).unwrap();
    light.set_intensity(1.0);
    light.set_position(&(scene_center - 2.0f32 * scene_radius * dir));
    light.set_direction(&dir);
    light.enable_shadows();

    dir = vec3(1.0, -1.0, -1.0).normalize();
    light = renderer.spot_light(1).unwrap();
    light.set_intensity(1.0);
    light.set_position(&(scene_center - 2.0f32 * scene_radius * dir));
    light.set_direction(&dir);
    light.enable_shadows();

    dir = vec3(1.0, -1.0, 1.0).normalize();
    light = renderer.spot_light(2).unwrap();
    light.set_intensity(1.0);
    light.set_position(&(scene_center - 2.0f32 * scene_radius * dir));
    light.set_direction(&dir);
    light.enable_shadows();

    dir = vec3(-1.0, -1.0, 1.0).normalize();
    light = renderer.spot_light(3).unwrap();
    light.set_intensity(1.0);
    light.set_position(&(scene_center - 2.0f32 * scene_radius * dir));
    light.set_direction(&dir);
    light.enable_shadows();

    let mut camera_handler = camerahandler::CameraHandler::new(camerahandler::CameraState::SPHERICAL);

    let mut weights: Option<HashMap<VertexID, Vec3>> = None;
    // main loop
    window.render_loop(move |events, _elapsed_time|
    {
        for event in events {
            match event {
                Event::Key {state, kind} => {
                    if kind == "Tab" && *state == State::Pressed
                    {
                        camera_handler.next_state();
                    }
                },
                Event::MouseClick {state, button, position} => {
                    if *button == MouseButton::Left
                    {
                        if *state == State::Pressed
                        {
                            let (x, y) = (position.0 / window_size.0 as f64, position.1 / window_size.1 as f64);
                            let p = renderer.camera.position();
                            let dir = renderer.camera.view_direction_at((x, y));
                            weights = on_click(&mesh,&vec3(p.x as f64, p.y as f64, p.z as f64), &vec3(dir.x as f64, dir.y as f64, dir.z as f64));
                            if weights.is_none() {
                                camera_handler.start_rotation();
                            }
                        }
                        else {
                            weights = None;
                            camera_handler.end_rotation()
                        }
                    }
                },
                Event::MouseWheel {delta} => {
                    camera_handler.zoom(&mut renderer.camera, *delta as f32);
                },
                Event::MouseMotion {delta} => {
                    camera_handler.rotate(&mut renderer.camera, delta.0 as f32, delta.1 as f32);
                    if let Some(ref w) = weights
                    {
                        on_morph(&mut mesh, w, 0.001 * delta.1);
                        let positions: Vec<f32> = mesh.positions_buffer().iter().map(|v| *v as f32).collect();
                        let normals: Vec<f32> = mesh.normals_buffer().iter().map(|v| *v as f32).collect();
                        wireframe_model.update_positions(&positions);
                        model.update_positions(&positions).unwrap();
                        model.update_normals(&normals).unwrap();
                    }
                }
            }
        }

        // Shadow pass
        renderer.shadow_pass(&|camera: &Camera| {
            mesh_shader.render(&model, &dust::Mat4::identity(), camera);
        });

        // Geometry pass
        renderer.geometry_pass(&|camera| {
            mesh_shader.render(&model, &dust::Mat4::identity(), camera);
            mesh_shader.render(&plane, &dust::Mat4::from_scale(100.0), camera);
            wireframe_model.render(camera);
        }).unwrap();

        // Light pass
        renderer.light_pass().unwrap();

        if let Some(ref path) = screenshot_path {
            #[cfg(target_arch = "x86_64")]
            save_screenshot(path, renderer.screen_rendertarget()).unwrap();
            std::process::exit(1);
        }
    }).unwrap();
}