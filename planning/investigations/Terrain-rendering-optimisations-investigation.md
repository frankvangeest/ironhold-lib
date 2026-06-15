Here is the full transcript of the video ***"When Your Game Is Bad But Your Optimisation Is Genius"*** by Vercidium:  
**\[[00:00](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=0)\]** Have you ever been excited to play a game only to be disappointed when it lags and stutters? I've spent the past 6 years creating a game engine and I've been shocked at the things that can make or break performance.

**\[[00:11](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=11)\]** I'll show three simple optimizations that you can use to make your game run up to 10 times quicker. And rather than just talking about it, we're going to optimize something you'll see in nearly every game: terrain. My current setup renders at 50 FPS and I'll show you how I increased it up to 400 using the first optimization.

**\[[00:30](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=30)\]** This optimization is all about triangles. You can make any kind of object out of them, and it's likely every game you've played is really just a bunch of triangles moving around. I've colored the model like this so you can see each triangle. Triangles can contain any kind of data you want, but storing a lot of data will hurt performance more than you might think. You don't need to sacrifice rendering quality for speed, but reducing memory usage will almost always guarantee faster rendering.

**\[[00:53](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=53)\]** To show you what I mean, let's look at how our terrain is rendered. It's composed of smaller meshes, and each mesh contains 128 triangles. Each triangle contains three vertices which contain position, normal, and UV vectors. This adds up to 12 kilobytes per mesh and 192 megabytes for the entire scene. We want to reduce this as much as we can.

**\[[01:16](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=76)\]** The first thing we'll do is get rid of the UV coordinates. These coordinates range from 0 to 1 and our position data ranges from 0 to 8\. This means we can divide the position by eight and use that to sample textures instead. With UVS gone, that brings each mesh down to 9 kilobytes and our terrain is now rendering at 75 FPS.

**\[[01:37](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=97)\]** The next step is to get rid of some position data. If we look at the terrain from above, all the meshes look the same. The only difference between them is their height. So why do we need to store the same X and Z positions in each one? What we can do instead is generate the positions in the vertex shader at runtime. To do this, we use a special variable called GL\_VertexID.

**\[[02:00](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=120)\]** This ID starts at zero for the first vertex then increments for each vertex after that. These vertices follow a pattern, first moving down and then up and to the right. Since this pattern repeats, we can recreate it in the shader using floor and mod functions. There's a bit more to the shader that I won't get into now as my plant generation video already covers it in detail. The important thing is that we no longer need to store the X and Z positions in every mesh. This boosts our FPS up to 130\.

**\[[02:29](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=149)\]** And the last thing we can get rid of is the normals. Currently, normals are stored as a 3D vector, but since they only represent a direction, they can be converted to pitch and yaw angles. This means we only need to store two floats, but we can compress them even further into 16-bit integers. This means they have half the precision, but that's still plenty for these angles. This reduces each mesh down to 3 kilobytes, and it's the smallest that we can make them without losing more precision.

**\[[02:53](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=173)\]** But there's one more way to reduce memory usage and make this run twice as fast. Currently, this mesh is made out of individual triangles, meaning we need to provide three vertices for each of them. We can also use triangle strips, which work a bit differently. We still need to provide three vertices for the first triangle, but every vertex after that builds a new triangle off the last. Both examples here use 12 vertices, but the strip produces six more triangles. If we change our mesh to use eight strips—one for each row—it reduces the amount of vertices we need down to 160\. This brings us down to 20 megabytes for the full scene, which is nearly 10 times less than what we started with.

**\[[03:29](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=209)\]** Our terrain is now rendering eight times faster, and there's two reasons for that. First, the memory bandwidth was the bottleneck in the initial approach. Our shaders would execute very quickly then wait around for more data to be read from memory. But by reducing our memory usage, our shaders can run constantly. The second reason is the vertex shader runs fewer times. Since we changed from individual triangles to strips, there are 2 and a half times less vertices to process.

**\[[04:01](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=241)\]** Now we can make this run even faster using the second optimization trick that I call batching. This is not unique to terrain and it's not the same as instancing. I'll explain it using ice cream. Imagine you walk into an ice cream shop with your 12 friends and you ask for an ice cream. Then you ask for another ice cream, and another ice cream. I know that I want 12 ice creams, but this ice cream dude has no idea. The ice cream dude is our GPU and each ice cream is a mesh that we want to draw. This is painfully slow. But GPUs are pretty quick, so drawing 12 meshes one at a time is all right, but if you try to render 100,000 of them this way, your GPU will explode.

**\[[04:41](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=281)\]** The solution is to write our 12 ice creams on a list and then hand it to the ice cream dude. He then hands us 12 ice creams, and the important thing here is that we only ever talked to him once, but in return, he gave us multiple things. To do this in our games, we give the GPU a list of all of our meshes and ask it to draw all of them. But we also need to tell the GPU where we want each mesh to be drawn. We can do this using a Shader Storage Buffer Object, which is an array in the shader that stores the positions of all of our meshes. We can select the right position for each mesh using another inbuilt variable called GL\_DrawID. This ID is similar to GL\_VertexID, but it's unique per mesh rather than per vertex.

**\[[05:23](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=323)\]** We now have an efficient way of rendering multiple meshes using only one draw call. So let's compare FPS with and without batching. So, why has the FPS stayed the same? To find out, let's have a look at two more metrics. The difference here is CPU time. In the old approach, the CPU spends more time sending commands to the GPU to draw every single mesh. But when we use batching, the CPU only has to send one command. The reason the FPS is the same is because the GPU is the bottleneck in both scenarios, but now the CPU is free to run other code.

**\[[05:53](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=353)\]** Next, we'll render more terrain. Why is this so slow? We're only rendering a billion triangles. I think I see the problem. This is where the third optimization trick is useful because it reduces the amount of triangles in this scene. Having a lot of triangles is great for detailed terrain, but if the player is standing here, that mountain in the distance will look the same if it has a thousand or a million triangles. Our goal is to use less triangles for distant terrain without the player noticing.

**\[[06:23](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=383)\]** Currently, we're rendering small meshes all the way out into the distance, so first we'll set a cap on this—and this is what we'll call our first level of detail. Then we'll render the terrain again, but with each mesh scaled up twice as big. This is our second level of detail. If we repeat this four times, our terrain will be a kilometer wide and only use 160,000 triangles. If we try doing this using only one level of detail, we'd have to use 8.3 million triangles.

**\[[06:48](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=408)\]** The best part about this is that each level of detail uses the same amount of triangles and takes the same amount of time to render. But when we try this in the 3D view, something strange occurs. Watch what happens when more levels are rendered. Since each level is rendered in the same position, the lower quality distant terrain is covering up the high quality close-up terrain. If we have a look at a cross-section, you can see the first level of detail in the middle and the other levels extending out to the sides. There are some places where the larger level of detail cuts across the smaller level, which hides our high-quality terrain.

**\[[07:25](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=445)\]** To fix this, most games will fade out low-quality objects the closer they are, but transparency is expensive on large objects like terrain. What we'll do instead is sink the terrain that's close to the player. As the player moves around, new high-quality terrain is generated underground and then rises to the surface. As a final optimization, we'll skip rendering the meshes that are completely underground. Let's go back to our 3D view and apply sinking. Now we can see the close-up, high-quality terrain.

**\[[07:58](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=478)\]** To show you how big our terrain is now, I'll highlight each level of detail and zoom out. Since the largest level of detail sinks, it feels like we're looking at a planet. Now, I can't compare FPS with the last approach because I don't have enough memory to render it without multiple levels of detail, but I can show the difference in size. This is the entire terrain that we're rendering now, and this is the terrain we're rendering at the start. Can you see it? I'll zoom in. This tiny blue area was rendering at 50 FPS, but now we're rendering 16,000 times more terrain at five times the FPS.

**\[[08:35](https://www.youtube.com/watch?v=5zlfJW2VGLM&t=515)\]** As a final step, we should disguise where each level of detail begins using foliage. As the camera moves around, you can see where the new high-quality terrain rises up. We'll need a lot of foliage to cover it up, and this video on screen will show you how to render highly optimized plants.

**Question:**   
Can you compact those lessons in a How to for a WGSL shader in rust bevy 0.18?

**Answer:**

Bringing Vercidium's low-level OpenGL/C++ optimizations into Bevy's highly parallel, wgpu-driven Render World is an excellent exercise in high-performance graphics programming.  
In Bevy, the engine already enforces **Lesson 2 (Batching)** out of the box using internal draw-command multi-draw / instancing systems. However, **Lesson 1 (Data Compression & Procedural Geometry)** and **Lesson 3 (Level of Detail via Geo-Sinking)** are entirely up to how you write your custom Material pipelines.  
Here is your practical architectural guide to building an hyper-optimized terrain renderer using Rust (Bevy) and WGSL.

## **1\. The WGSL Shader (terrain.wgsl)**

To optimize the bandwidth bottleneck, we discard explicit UV buffers and compute standard X and Z vertex positions entirely from the built-in @builtin(vertex\_index) hook (GL\_VertexID equivalent). We expect a tight vertex buffer containing only a compressed normal structure and a scalar height.

**Code snippet**
``` shader  
\#import bevy\_pbr::mesh\_view\_bindings as view\_bindings

struct VertexInput {  
    @builtin(vertex\_index) vertex\_id: u32,  
    @location(0) height: f32,  
    // Compressed Normals: Pack Yaw/Pitch into two 16-bit integers packed as a u32  
    @location(1) packed\_angles: u32,   
};

struct VertexOutput {  
    @builtin(position) clip\_position: vec4\<f32\>,  
    @location(0) world\_position: vec3\<f32\>,  
    @location(1) normal: vec3\<f32\>,  
};

struct TerrainUniforms {  
    chunk\_size: f32,  
    grid\_width: u32,  
    lod\_level: f32,  
    player\_pos: vec3\<f32\>,  
};

@group(2) @binding(0) var\<uniform\> config: TerrainUniforms;

@vertex  
fn vertex(input: VertexInput) \-\> VertexOutput {  
    var out: VertexOutput;

    // \--- LESSON 1: PROCEDURAL GEOMETRY FROM INDEX \---  
    // Reconstructing a grid mesh topology procedurally using triangle strips logic  
    let x\_index \= input.vertex\_id % config.grid\_width;  
    let z\_index \= input.vertex\_id / config.grid\_width;  
      
    var local\_x \= f32(x\_index) \* config.chunk\_size;  
    var local\_z \= f32(z\_index) \* config.chunk\_size;  
      
    // \--- LESSON 3: GEO-SINKING FOR LOD TRANSITIONS \---  
    // Calculate distance to player to conditionally sink boundary vertices underground  
    let mesh\_world\_origin \= vec3\<f32\>(local\_x, input.height, local\_z);   
    let dist \= distance(mesh\_world\_origin.xz, config.player\_pos.xz);  
      
    var final\_height \= input.height;  
    let max\_distance \= 500.0 \* (config.lod\_level \+ 1.0);  
      
    if (dist \> (max\_distance \* 0.85)) {  
        // Linearly sink into oblivion towards the edge of this LOD tier  
        let t \= (dist \- (max\_distance \* 0.85)) / (max\_distance \* 0.15);  
        final\_height \-= saturate(t) \* 150.0;   
    }

    let world\_pos \= vec4\<f32\>(local\_x, final\_height, local\_z, 1.0);  
    out.world\_position \= world\_pos.xyz;  
    out.clip\_position \= view\_bindings::view.view\_proj \* world\_pos;

    // \--- LESSON 1: DECOMPRESS ANGLES TO NORMALS \---  
    let yaw \= f32(input.packed\_angles & 0xFFFFu) / 65535.0 \* 6.28318;  
    let pitch \= f32((input.packed\_angles \>\> 16u) & 0xFFFFu) / 65535.0 \* 3.14159;  
      
    out.normal \= vec3\<f32\>(  
        cos(pitch) \* sin(yaw),  
        sin(pitch),  
        cos(pitch) \* cos(yaw)  
    );

    return out;  
}

@fragment  
fn fragment(in: VertexOutput) \-\> @location(0) vec4\<f32\> {  
    // Procedural UV calculation: Position / Scale  
    let tex\_uv \= in.world\_position.xz / 8.0;   
      
    // Simple directional lighting with normal  
    let light\_dir \= normalize(vec3\<f32\>(0.5, 1.0, 0.3));  
    let diffuse \= max(dot(in.normal, light\_dir), 0.2);  
      
    let base\_color \= vec3\<f32\>(0.2, 0.5, 0.2); // Green grass  
    return vec4\<f32\>(base\_color \* diffuse, 1.0);  
}
``` 

## **2\. The Rust Host Implementation (main.rs)**

To feed this shader safely in Bevy, we implement a custom pipeline via Material and AsBindGroup. Instead of populating Mesh::ATTRIBUTE\_POSITION, we explicitly design minimal custom attributes to drastically cut memory bandwidth.  

**Rust code**  
``` rust
use bevy::{  
    prelude::\*,  
    reflect::TypePath,  
    render::{  
        mesh::{MeshVertexAttribute, VertexAttributeValues},  
        render\_resource::{AsBindGroup, ShaderRef, VertexFormat},  
    },  
};

// Define explicit low-bandwidth vertex layouts  
pub const ATTRIBUTE\_HEIGHT: MeshVertexAttribute \=  
    MeshVertexAttribute::new("TerrainHeight", 98324701, VertexFormat::Float32);

pub const ATTRIBUTE\_PACKED\_ANGLES: MeshVertexAttribute \=  
    MeshVertexAttribute::new("PackedAngles", 98324702, VertexFormat::Uint32);

fn main() {  
    App::new()  
        .add\_plugins((DefaultPlugins, MaterialPlugin::\<TerrainMaterial\>::default()))  
        .add\_systems(Startup, setup)  
        .add\_systems(Update, update\_lod\_uniforms)  
        .run();  
}

\#\[derive(AsBindGroup, Asset, TypePath, Clone)\]  
pub struct TerrainMaterial {  
    \#\[uniform(0)\]  
    pub config: TerrainUniforms,  
}

\#\[derive(Copy, Clone, ShaderType, GLSLType, Debug)\]  
pub struct TerrainUniforms {  
    pub chunk\_size: f32,  
    pub grid\_width: u32,  
    pub lod\_level: f32,  
    pub player\_pos: Vec3,  
}

impl Material for TerrainMaterial {  
    fn vertex\_shader() \-\> ShaderRef {  
        "shaders/terrain.wgsl".into()  
    }  
    fn fragment\_shader() \-\> ShaderRef {  
        "shaders/terrain.wgsl".into()  
    }  
}

fn setup(  
    mut commands: Commands,  
    mut meshes: ResMut\<Assets\<Mesh\>\>,  
    mut materials: ResMut\<Assets\<TerrainMaterial\>\>,  
) {  
    // Generate an ultra-lean custom compressed mesh topology  
    let mut mesh \= Mesh::new(  
        bevy::render::render\_resource::PrimitiveTopology::TriangleStrip,   
        bevy::render::settings::RenderAssetUsages::default()  
    );

    let mut heights \= Vec::new();  
    let mut packed\_angles \= Vec::new();  
      
    let grid\_size \= 64;  
    for z in 0..grid\_size {  
        for x in 0..grid\_size {  
            // Sample a basic procedural height map  
            let h \= ((x as f32 \* 0.1).sin() \+ (z as f32 \* 0.1).cos()) \* 4.0;  
            heights.push(h);

            // Compress standard Normal (0.0, 1.0, 0.0) up into raw angles  
            let yaw: f32 \= 0.0;   
            let pitch: f32 \= 1.5707; // Upwards facing  
            let packed\_yaw \= ((yaw / 6.28318) \* 65535.0) as u32;  
            let packed\_pitch \= ((pitch / 3.14159) \* 65535.0) as u32;  
            let packed \= packed\_yaw | (packed\_pitch \<\< 16);  
              
            packed\_angles.push(packed);  
        }  
    }

    // Allocate minimal specific buffers (No structural X/Z array data, no explicit explicit UVs)  
    mesh.insert\_attribute(ATTRIBUTE\_HEIGHT, heights);  
    mesh.insert\_attribute(ATTRIBUTE\_PACKED\_ANGLES, packed\_angles);

    // Build standard programmatic index list matching a continuous Triangle Strip sequence  
    let mut indices \= Vec::new();  
    for z in 0..(grid\_size \- 1) {  
        for x in 0..grid\_size {  
            indices.push(z \* grid\_size \+ x);  
            indices.push((z \+ 1) \* grid\_size \+ x);  
        }  
        // Inject degenerate triangles to jump rows cleanly without artifacts  
        if z \< grid\_size \- 2 {  
            indices.push((z \+ 1) \* grid\_size \+ (grid\_size \- 1));  
            indices.push((z \+ 1) \* grid\_size);  
        }  
    }  
    mesh.insert\_indices(bevy::render::mesh::Indices::U32(indices));

    // Spawn chunk handling  
    commands.spawn((  
        Mesh3d(meshes.add(mesh)),  
        MeshMaterial3d(materials.add(TerrainMaterial {  
            config: TerrainUniforms {  
                chunk\_size: 1.0,  
                grid\_width: grid\_size,  
                lod\_level: 0.0,  
                player\_pos: Vec3::ZERO,  
            },  
        })),  
        Transform::from\_xyz(0.0, 0.0, 0.0),  
    ));

    // Camera  
    commands.spawn((  
        Camera3d::default(),  
        Transform::from\_xyz(0.0, 20.0, 50.0).looking\_at(Vec3::ZERO, Vec3::Y),  
    ));  
}

// Drive uniform tracking per frame to allow real-time boundary morphing   
fn update\_lod\_uniforms(  
    camera\_q: Query\<\&Transform, With\<Camera3d\>\>,  
    material\_handles: Query\<\&MeshMaterial3d\<TerrainMaterial\>\>,  
    mut materials: ResMut\<Assets\<TerrainMaterial\>\>,  
) {  
    if let Ok(cam\_trans) \= camera\_q.get\_single() {  
        for handle in material\_handles.iter() {  
            if let Some(mat) \= materials.get\_mut(handle) {  
                mat.config.player\_pos \= cam\_trans.translation;  
            }  
        }  
    }  
}
```

## **Why this scales efficiently inside Bevy**

1. **Memory Reduction (Lesson 1):** Traditional unoptimized configurations allocate up to $48\\text{ bytes}$ per terrain vertex ($3\\times\\text{f32 Position}$, $3\\times\\text{f32 Normal}$, $2\\times\\text{f32 UV}$). With this method, you transmit exactly **$8\\text{ bytes}$** ($1\\times\\text{f32 Height} \+ 1\\times\\text{u32 packed angle}$). The rest is computed safely directly inside the L1/L2 texture memory caches via runtime arithmetic functions.  
2. **Native Batching (Lesson 2):** Because your chunks share a single underlying component architecture layout (TerrainMaterial), Bevy automatically leverages **Frustum Culling** on the CPU to ignore hidden components, and automatically bundles visible instances into high-efficiency clustered MultiDrawIndexedIndirect GPU calls behind the scenes.  
3. **Seamless Tiling (Lesson 3):** Rather than writing expensive Alpha-blended translucent pixel passes across geometry tiers, standard distant terrain bounds organically dip into negative global thresholds away from the relative center context. This hides high-to-low resolution pop-in behind organic foliage clutter or foggy horizons effortlessly.


What lessons are good to apply the ironhold's terrain generation?
Should we break it up in multiple features?