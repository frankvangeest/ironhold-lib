

Markdown  
\# Instruction Guide: Implementing Anime Foliage (Stylized Trees) in WebAssembly using Rust and Bevy 0.18

This document serves as an explicit instruction file for an AI agent to implement a highly performant, stylized anime-style tree engine in a Bevy 0.18 game engine configured for WebAssembly (WASM). 

The content is adapted from the procedural pipeline outlined in the tutorial \[Anime Tree Tutorial | Blender\](http://www.youtube.com/watch?v=52sTppv7Y-E).

\---

\#\# 1\. Architectural Blueprint Overview

Traditional 3D foliage struggles with stylized anime rendering because fixed-angle polygons create harsh, unpainted looking shadows and flat perspectives. This workflow bypasses this limitation using two primary mechanisms:  
1\. \*\*Camera-Facing Billboard Instancing:\*\* Leaf cards constantly orient themselves towards the camera, maintaining painterly shapes mimicking real canvas brush strokes.  
2\. \*\*Normal-Transferred Cell Shading:\*\* Vertex normals are decoupled from individual leaf geometries and instead mapped to an enclosing spherical hull. This ensures large, continuous light-and-shadow volumes rather than fragmented lighting on individual cards.

Because the final compilation target is \*\*WASM\*\*, the agent must strictly prioritize GPU execution (via custom WGSL shaders and GPU instancing) to keep the CPU main loop lightweight.

\---

\#\# 2\. Pipeline Implementation Steps

\#\#\# Step 2.1: Leaf Mesh Billboard Geometry (Bevy 0.18 / WGSL)  
Instead of processing camera rotation per-instance via a CPU systems loop—which introduces critical scaling bottlenecks under WASM—the billboard alignment must be executed inside the \*\*WGSL Vertex Shader\*\*.

\* \*\*Instruction for Agent:\*\* Generate an asset loader or mesh component that creates a simple 2D Plane mesh representing a singular leaf/brush stroke.  
\* \*\*Vertex Shader Logic:\*\* Inside the vertex shader, strip the rotation from the \`ModelToWorld\` matrix (or extract the camera's right and up vectors from the \`View\` uniform bind group) to ensure the quad vertices expand relative to the view plane rather than world orientations.

\#\#\# Step 2.2: Procedural Cluster Emitter (The "Foliage Bush")  
The tree is assembled using composite structural units ("bushes"). Each cluster uses a base mesh emitter to distribute individual leaf cards.

\* \*\*Instruction for Agent:\*\* Implement an instancing system using Bevy’s \`ExtractInstances\` and custom pipeline bindings.   
\* \*\*Data Generation:\*\* Create a sphere mesh or evenly subdivided cube hull to act as the spatial anchor (\`emitter\`). Sample points randomly or uniformly across the faces of this hull.  
\* \*\*Instance Uniforms:\*\* For each sampled position, populate an instance array containing:  
    \* \`position\`: The sampled face position.  
    \* \`scale\`: Randomly fluctuated between a defined bounds vector to introduce organic size scaling variations.

\#\#\# Step 2.3: Vertex Normal Modification & Normal Transfer  
To achieve uniform toon shading across hundreds of discrete leaf cards, their vertex normals must mimic a smooth, collective volume instead of pointing in their localized quad coordinates.

\* \*\*Instruction for Agent:\*\* Write a procedural computation pass in Rust during the mesh extraction stage.  
\* \*\*Algorithm:\*\* 1\. For every leaf card instance belonging to a specific cluster emitter, determine its center point in local space.  
    2\. Calculate a vector pointing from the center of the base \`emitter\` hull towards that specific leaf center point.  
    3\. Normalize this vector and assign it as the explicit vertex normal (\`Vertex\_Attribute\_Normal\`) for all four vertices of that leaf card quad.  
\* \*\*Result:\*\* The entire cluster will now reflect light as if it were a perfectly smooth sphere, producing unbroken, contiguous shadows.

\#\#\# Step 2.4: The Toon Cell Shader with Depth Enhancements (WGSL)  
Replace standard PBR lighting equations with a custom \`Material2d\` or \`Material\` implementation utilizing a hard-stepped ramp.

\`\`\`wgsl  
// Core Shading Snippets for the AI Agent's WGSL compilation:

struct FragmentInput {  
    @builtin(position) frag\_coord: vec4f,  
    @location(0) world\_position: vec4f,  
    @location(1) world\_normal: vec3f,  
    @location(2) uv: vec2f,  
};

@group(2) @binding(0) var alpha\_texture: texture\_2d\<f32\>;  
@group(2) @binding(1) var texture\_sampler: sampler;

// Cell Shading & Toon-Ramp logic  
fn calculate\_cel\_shading(light\_dir: vec3f, normal: vec3f) \-\> f32 {  
    let NdotL \= dot(normalize(normal), normalize(light\_dir));  
      
    // Discrete stepped thresholds imitating an art canvas  
    if (NdotL \> 0.5) {  
        return 1.0; // Highlight  
    } else if (NdotL \> 0.0) {  
        return 0.6; // Midtone  
    } else {  
        return 0.2; // Shadow Accent  
    }  
}  
\`\`\`

* Alpha Masking: Sample an alpha\_texture mapped to painterly brush-strokes. Use a hard discard statement (if alpha \< 0.5 { discard; }) mimicking an Alpha Clip function to preserve clean silhouettes without handling expensive alpha blending ordering in WebGL2/WASM.  
* Ambient Occlusion (AO) Simulation: In WebGL2 WASM contexts where screen-space ambient occlusion (SSAO) might be costly, bake or inject low-frequency darkness factors. Apply a mathematical power function (pow(ambient, intensity)) to multiply against shadows, accentuating deep internal splits between branches.  
* Light-Synchronized Gradient Mapping: Pass the directional Sun Light vector into the fragment shader. Construct a world-space linear gradient running parallel with the light ray path, multiplying the overall color by a dark value on the shadowed hemisphere to maximize depth consistency.

### **Step 2.5: Algorithmic Tree Synthesis (Entity Placement)**

Construct the final macro-shape of the tree by stacking and grouping the components procedurally or via scene graphs.

* Instruction for Agent: Implement a structural spawner entity loop.  
* Hierarchy: Create a parent entity representing the structural Trunk/Banches (using a low-poly stylized model). Attach multiple "Foliage Bush" entities as child structures. Scale, shift, and jitter the parameters of each child cluster to match a target reference silhouette while sharing the exact same instance rendering pipeline.

## **3\. High Performance WebAssembly Optimization Directives**

To maintain target framerates on low-overhead platforms via WASM, the AI Agent must strictly apply the following operational constraints to its code generation output:

1. Zero-Allocation Per Frame: Ensure that particle offsets, instance arrays, and layout transforms are stored in GPU buffers (BufferVec or Storage Buffers) and updated only when the tree layout scales or mutates structurally.  
2. Explicit Frustum Culling: Leverage Bevy’s built-in AABB components. Ensure that even though vertex transformations occur in the WGSL shader, the custom pipeline assigns accurate bounding volumes covering the entire expanded cluster to prevent premature GPU culling.  
3. Draw Call Minimization: All clusters sharing identical material colors and leaf texture maps must be packed seamlessly into a unified storage buffer to enable a single instanced draw execution call per frame.

## **4\. Extended Artistic Effects Architecture**

### **Vertex-Based GPU Wind Simulation**

Do not process wind positions using individual entity translations on the CPU.

* Implementation: In the custom WGSL vertex shader, introduce a time uniform loop (globals.time). Pass a low-frequency 2D Noise Texture or calculate simple combined Sine/Cosine waves inside the vertex stage.  
* Transformation: Apply a dual-tier offset calculation:  
  1. Micro-Sway: Apply a high-frequency, low-amplitude rotational flutter to individual vertex corners based on vertex IDs to simulate leaves rustling.  
  2. Macro-Sway: Apply a low-frequency, high-amplitude positional offset down the tree hierarchy based on vertex height coordinates (world\_position.y) to simulate large branch movements bending to the breeze.

### **Particle Leaf Drop Engine**

* Implementation: Utilize a simplified GPU computation path or low-footprint compute step to emit falling individual quad entities from the bounded zones of the leaf cluster entities, applying gravitational downward velocity and a persistent rotational twist before alpha fading at terminal lifetimes.