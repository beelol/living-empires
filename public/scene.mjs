import * as THREE from '/vendor/three.js';
import {THEMES,fixtureRecipe,moveCommander,accessScenario} from './core.mjs';

export function createWorld(container,{audit=false,reviewHalfExtent}={}) {
  const scene=new THREE.Scene();
  const camera=new THREE.PerspectiveCamera(40,1,.1,200);
  const renderer=new THREE.WebGLRenderer({antialias:true,preserveDrawingBuffer:true});
  renderer.setPixelRatio(Math.min(devicePixelRatio,2));renderer.shadowMap.enabled=true;renderer.shadowMap.type=THREE.PCFSoftShadowMap;
  renderer.toneMapping=THREE.ACESFilmicToneMapping;renderer.toneMappingExposure=1.35;
  container.appendChild(renderer.domElement);
  const world=new THREE.Group();scene.add(world);
  scene.add(new THREE.HemisphereLight(0xf4f9ed,0x344c46,3));
  const sun=new THREE.DirectionalLight(0xfff2d5,4);sun.position.set(-12,20,10);sun.castShadow=true;sun.shadow.mapSize.set(2048,2048);Object.assign(sun.shadow.camera,{left:-28,right:28,top:28,bottom:-28});scene.add(sun);
  const mat=(color,extra={})=>new THREE.MeshStandardMaterial({color,roughness:.82,...extra});
  const architecturalMat=(role,color,extra={})=>{
    const family=theme?.architecture==='organic'?{roughness:.95,metalness:0}:theme?.architecture==='fortified'?{roughness:role==='shell'?1:.65,metalness:role==='frame'?.3:0}:{roughness:role==='shell'?.45:role==='frame'?.4:.82,metalness:role==='frame'?.65:0};
    return mat(color,{...family,...extra});
  };
  function mesh(geometry,material,position,parent=world){const m=new THREE.Mesh(geometry,material);m.position.set(...position);m.castShadow=true;m.receiveShadow=true;parent.add(m);return m;}
  let halfExtent=4;
  const views=()=>[[halfExtent*1.85,halfExtent*1.65,halfExtent*2.35],[-halfExtent*1.9,halfExtent*1.3,halfExtent*1.65],[0,halfExtent*.85,halfExtent*2.5]];
  let recipe,theme,commander,ring,sensor,angle=0,currentView=0,position={x:0,z:9},target=null,last=performance.now(),disposed=false;
  const disposeGroup=()=>{while(world.children.length){const child=world.children[0];child.traverse(o=>{o.geometry?.dispose();if(o.material)for(const m of Array.isArray(o.material)?o.material:[o.material])m.dispose();});world.remove(child);}};
  function load(next=fixtureRecipe(),nextTheme=THEMES[next.themeId]){
    disposeGroup();recipe=next;theme=nextTheme;position={x:0,z:9};target=null;
    const recipeExtent=Number.isFinite(recipe.scene?.halfExtent)?recipe.scene.halfExtent:4;
    halfExtent=audit?(reviewHalfExtent??4):recipeExtent;
    if(audit&&recipe.scene&&Math.abs(recipe.scene.halfExtent-halfExtent)>.001)throw new Error('Candidate scene extent differs from the frozen review extent.');
    scene.background=new THREE.Color(theme.sky);scene.fog=new THREE.Fog(theme.sky,35,85);
    const groundRadius=halfExtent+19;
    mesh(new THREE.CylinderGeometry(groundRadius,groundRadius+1,1.4,80),mat(theme.palette.ground),[0,-.75,0]);
    mesh(new THREE.CylinderGeometry(groundRadius+1,groundRadius-4,3,12),mat(theme.palette.frame),[0,-2.7,0]);
    // Fixed seeded terrain: scene composition is repeatable across candidate reviews.
    let seed=27;const rand=()=>{seed=(seed*16807)%2147483647;return(seed-1)/2147483646;};
    for(let i=0;i<110;i++){
      const a=rand()*Math.PI*2,r=halfExtent+3+rand()*14,x=Math.cos(a)*r,z=Math.sin(a)*r;
      if(z>4&&Math.abs(x)<2.5)continue;
      const s=.25+rand()*1.3;
      const rock=mesh(new THREE.DodecahedronGeometry(s,0),mat(theme.palette.rock),[x,s*.35,z]);rock.scale.set(1,.6,1);rock.rotation.set(rand(),rand(),0);
      if(i%4===0){
        if(theme.flora==='tree'){
          mesh(new THREE.CylinderGeometry(.12,.2,2.4,6),architecturalMat('frame',theme.palette.frame),[x,1.2,z]);
          mesh(new THREE.ConeGeometry(1,3,7),mat(theme.palette.ground),[x,3,z]);
        }else{
          const crystal=mesh(new THREE.ConeGeometry(s*.35,s*3,theme.flora==='basalt'?6:5),mat(theme.flora==='basalt'?theme.palette.frame:theme.palette.accent,{emissive:theme.palette.accent,emissiveIntensity:theme.flora==='basalt'?.02:.15}),[x,s*1.4,z]);crystal.rotation.z=(rand()-.5)*.3;
        }
      }
    }
    const foundation=mesh(new THREE.CylinderGeometry(halfExtent+1.2,halfExtent+1.5,.14,6),architecturalMat('frame',theme.palette.frame),[0,-.03,0]);foundation.receiveShadow=true;
    for(const p of recipe.parts){
      const geometry=p.shape==='box'?new THREE.BoxGeometry(...p.size):p.shape==='cylinder'?new THREE.CylinderGeometry(p.size[0]/2,p.size[2]/2,p.size[1],12):new THREE.ConeGeometry(p.size[0]/2,p.size[1],6);
      mesh(geometry,architecturalMat(p.material,theme.palette[p.material],p.material==='accent'?{emissive:theme.palette.accent,emissiveIntensity:.45}:{}),p.position);
    }
    const pathMaterial=mat(theme.palette.rock);
    for(let z=5;z<Math.max(17,halfExtent+4);z+=1.1)mesh(new THREE.BoxGeometry(1.3,.06,.7),pathMaterial,[0,.03,z]);
    sensor=mesh(new THREE.TorusGeometry(recipe.sensorRadius,.025,6,100),new THREE.MeshBasicMaterial({color:theme.palette.accent,transparent:true,opacity:.28}),[0,.07,0]);sensor.rotation.x=Math.PI/2;
    // A visible signal target exercises the declared sensor radius.
    mesh(new THREE.CylinderGeometry(.25,.45,1,6),architecturalMat('frame',theme.palette.frame),[8,.5,0]);
    mesh(new THREE.IcosahedronGeometry(.35),mat(theme.palette.accent,{emissive:theme.palette.accent,emissiveIntensity:1}),[8,1.3,0]);
    commander=new THREE.Group();world.add(commander);
    mesh(new THREE.BoxGeometry(.65,.75,.6),architecturalMat('shell',theme.palette.shell),[0,.7,0],commander);
    mesh(new THREE.BoxGeometry(.45,.35,.4),architecturalMat('frame',theme.palette.frame),[0,1.25,0],commander);
    mesh(new THREE.BoxGeometry(.3,.08,.03),mat(theme.palette.accent,{emissive:theme.palette.accent,emissiveIntensity:1}),[0,1.28,.21],commander);
    for(const x of [-.22,.22])mesh(new THREE.BoxGeometry(.18,.4,.25),architecturalMat('frame',theme.palette.frame),[x,.2,0],commander);
    ring=mesh(new THREE.TorusGeometry(.65,.035,6,32),new THREE.MeshBasicMaterial({color:theme.palette.accent}),[0,.05,0],commander);ring.rotation.x=Math.PI/2;
    commander.position.set(position.x,0,position.z);view(0);render();
  }
  function view(index){
    currentView=index%3;
    const [x,y,z]=views()[currentView];
    const rect=container.getBoundingClientRect(),aspect=rect.height>0?rect.width/rect.height:1;
    const fit=audit?1:Math.max(1,1.35/aspect);
    camera.position.set(x*fit,1.4+(y-1.4)*fit,z*fit);camera.lookAt(0,1.4,0);
    if(!audit){
      scene.fog.near=35*fit;scene.fog.far=85*fit;
      camera.far=Math.max(200,Math.hypot(x*fit,(y-1.4)*fit,z*fit)+halfExtent+30);
      camera.updateProjectionMatrix();
    }
    render();
  }
  function render(){renderer.render(scene,camera);}
  function resize(){const r=container.getBoundingClientRect();renderer.setSize(Math.max(1,r.width),Math.max(1,r.height));camera.aspect=r.width/Math.max(1,r.height);camera.updateProjectionMatrix();view(currentView);}
  const observer=new ResizeObserver(resize);observer.observe(container);
  const raycaster=new THREE.Raycaster(),plane=new THREE.Plane(new THREE.Vector3(0,1,0),0);
  renderer.domElement.addEventListener('pointerdown',e=>{
    if(audit)return;
    const r=renderer.domElement.getBoundingClientRect();raycaster.setFromCamera(new THREE.Vector2((e.clientX-r.left)/r.width*2-1,-(e.clientY-r.top)/r.height*2+1),camera);
    const point=new THREE.Vector3();raycaster.ray.intersectPlane(plane,point);if(Math.hypot(point.x,point.z)<26)target={x:point.x,z:point.z};
  });
  function tick(now){if(disposed)return;const dt=Math.min((now-last)/1000,.05);last=now;angle+=dt;
    if(target){position=moveCommander(recipe,position,target,dt);commander.position.set(position.x,0,position.z);if(Math.hypot(position.x-target.x,position.z-target.z)<.1)target=null;}
    render();requestAnimationFrame(tick);
  }
  load();resize();requestAnimationFrame(tick);
  return {load,view,render,goInside(){target={x:0,z:0};},traverse(){const result=accessScenario(recipe);position=result.position;commander.position.set(position.x,0,position.z);render();return result;},dispose(){disposed=true;observer.disconnect();disposeGroup();renderer.dispose();}};
}
