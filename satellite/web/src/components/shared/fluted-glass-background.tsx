import { useEffect, useRef } from "react"

import { cn } from "@/lib/utils"

/**
 * Procedural blue fluted-glass WebGL background.
 * Ported from the standalone LeiBox demo (reconstruction of Sasha Birukoff's look).
 */
const VERTEX = `attribute vec2 position;void main(){gl_Position=vec4(position,0.,1.);}`

const FRAGMENT = `
precision highp float;
uniform vec2 resolution;
uniform float time;
float blob(vec2 p,vec2 center,vec2 radius,float angle){
  p-=center;float c=cos(angle),s=sin(angle);
  p=mat2(c,-s,s,c)*p/radius;
  return exp(-dot(p,p));
}
vec3 fluid(vec2 p,float t){
  float s=sin(t),c=cos(t);
  p.x+=.025*sin(p.y*7.0+t*2.0);
  float trunk=blob(p,vec2(.52+.15*s-.07*sin(t*2.0),.76),vec2(.235,.46),-.12-.52*s);
  float crown=blob(p,vec2(.77-.55*s,.20-.10*sin(t*2.0)),vec2(.235,.23),-.50+.45*s);
  float baseY=1.045-.17*p.x+.075*sin(p.x*5.0+t*2.0);
  float base=exp(-pow((p.y-baseY)/.092,2.0));
  float hole=blob(p,vec2(.85-.55*s,.59-.13*s),vec2(.19+.025*c,.32),.1+.5*s);
  float density=(trunk+.56*crown+.90*base)*(1.0-.78*hole);
  vec3 color=mix(vec3(.957),vec3(.66,.80,.89),smoothstep(.015,.20,density));
  color=mix(color,vec3(.10,.48,.73),smoothstep(.16,.49,density));
  color=mix(color,vec3(.015,.135,.28),smoothstep(.40,.73,density));
  color=mix(color,vec3(.008,.025,.087),smoothstep(.62,.93,density));
  return color;
}
void main(){
  vec2 uv=vec2(gl_FragCoord.x/resolution.x,1.0-gl_FragCoord.y/resolution.y);
  float rib=fract(uv.x*71.0)-.5;
  vec2 refracted=uv+vec2(rib*.018,(rib*rib-.0833)*.034);
  float t=time*6.28318530718/28.0;
  vec3 color=fluid(refracted,t)*.40;
  color+=fluid(refracted+vec2(0.,.025),t)*.24;
  color+=fluid(refracted-vec2(0.,.025),t)*.24;
  color+=fluid(refracted+vec2(0.,.055),t)*.06;
  color+=fluid(refracted-vec2(0.,.055),t)*.06;
  float edge=pow(abs(rib)*2.0,10.0);
  float blue=(1.0-color.r)*smoothstep(.02,.18,color.b-color.r);
  color+=vec3(.018,.025,.03)*(rib*.65+edge*.24)*blue;
  gl_FragColor=vec4(clamp(color,0.0,1.0),1.0);
}`

function createShader(
  gl: WebGLRenderingContext,
  type: number,
  source: string,
): WebGLShader | null {
  const shader = gl.createShader(type)
  if (!shader) return null
  gl.shaderSource(shader, source)
  gl.compileShader(shader)
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    gl.deleteShader(shader)
    return null
  }
  return shader
}

function paintFallback(canvas: HTMLCanvasElement) {
  const ctx = canvas.getContext("2d")
  if (!ctx) return
  const { width, height } = canvas
  ctx.fillStyle = "#f4f4f4"
  ctx.fillRect(0, 0, width, height)
  const gradient = ctx.createRadialGradient(
    width * 0.55,
    height * 0.78,
    20,
    width * 0.55,
    height * 0.78,
    Math.max(width, height) * 0.7,
  )
  gradient.addColorStop(0, "#020820")
  gradient.addColorStop(0.45, "#073d69")
  gradient.addColorStop(0.7, "#378fbe")
  gradient.addColorStop(1, "#f4f4f4")
  ctx.fillStyle = gradient
  ctx.fillRect(0, 0, width, height)
}

type FlutedGlassBackgroundProps = {
  className?: string
  /** When true, freezes the animation (still draws current frame). */
  paused?: boolean
}

export function FlutedGlassBackground({
  className,
  paused = false,
}: FlutedGlassBackgroundProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const pausedRef = useRef(paused)
  const syncRef = useRef<() => void>(() => {})
  pausedRef.current = paused

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const media = window.matchMedia("(prefers-reduced-motion: reduce)")
    let gl: WebGLRenderingContext | null = canvas.getContext("webgl", {
      alpha: false,
      antialias: false,
      depth: false,
      preserveDrawingBuffer: true,
    })
    let program: WebGLProgram | null = null
    let buffer: WebGLBuffer | null = null
    let timeLocation: WebGLUniformLocation | null = null
    let sizeLocation: WebGLUniformLocation | null = null
    let frame = 0
    let last = 0
    let elapsed = 0
    let visible = true
    let destroyed = false
    let useFallback = false

    function initialize(): boolean {
      if (!gl) return false
      const vertex = createShader(gl, gl.VERTEX_SHADER, VERTEX)
      const fragment = createShader(gl, gl.FRAGMENT_SHADER, FRAGMENT)
      if (!vertex || !fragment) {
        if (vertex) gl.deleteShader(vertex)
        if (fragment) gl.deleteShader(fragment)
        return false
      }
      program = gl.createProgram()
      if (!program) {
        gl.deleteShader(vertex)
        gl.deleteShader(fragment)
        return false
      }
      gl.attachShader(program, vertex)
      gl.attachShader(program, fragment)
      gl.linkProgram(program)
      gl.deleteShader(vertex)
      gl.deleteShader(fragment)
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) return false
      gl.useProgram(program)
      buffer = gl.createBuffer()
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
      gl.bufferData(
        gl.ARRAY_BUFFER,
        new Float32Array([-1, -1, 1, -1, -1, 1, -1, 1, 1, -1, 1, 1]),
        gl.STATIC_DRAW,
      )
      const position = gl.getAttribLocation(program, "position")
      gl.enableVertexAttribArray(position)
      gl.vertexAttribPointer(position, 2, gl.FLOAT, false, 0, 0)
      timeLocation = gl.getUniformLocation(program, "time")
      sizeLocation = gl.getUniformLocation(program, "resolution")
      return true
    }

    if (!initialize()) {
      if (gl) {
        if (program) gl.deleteProgram(program)
        gl.getExtension("WEBGL_lose_context")?.loseContext()
      }
      gl = null
      useFallback = true
    }

    function reduced() {
      return media.matches
    }

    function running() {
      return (
        !!gl &&
        !destroyed &&
        !useFallback &&
        !pausedRef.current &&
        !reduced() &&
        visible &&
        !document.hidden
      )
    }

    function render() {
      if (!gl || destroyed || !canvas) return
      gl.viewport(0, 0, canvas.width, canvas.height)
      gl.uniform2f(sizeLocation, canvas.width, canvas.height)
      gl.uniform1f(timeLocation, elapsed)
      gl.drawArrays(gl.TRIANGLES, 0, 6)
    }

    function tick(now: number) {
      frame = 0
      if (!running()) {
        last = 0
        return
      }
      if (last) elapsed += Math.min((now - last) / 1000, 0.05)
      last = now
      render()
      frame = requestAnimationFrame(tick)
    }

    function sync() {
      cancelAnimationFrame(frame)
      frame = 0
      last = 0
      if (useFallback || !gl) {
        paintFallback(canvas)
        return
      }
      render()
      if (running()) frame = requestAnimationFrame(tick)
    }
    syncRef.current = sync

    function resize() {
      const bounds = canvas.getBoundingClientRect()
      const dpr = Math.min(window.devicePixelRatio || 1, 1.5)
      canvas.width = Math.max(1, Math.round(bounds.width * dpr))
      canvas.height = Math.max(1, Math.round(bounds.height * dpr))
      sync()
    }

    const resizeObserver = new ResizeObserver(resize)
    resizeObserver.observe(canvas)
    const intersection = new IntersectionObserver((entries) => {
      visible = entries[0]?.isIntersecting ?? false
      sync()
    })
    intersection.observe(canvas)

    media.addEventListener("change", sync)
    document.addEventListener("visibilitychange", sync)
    resize()

    return () => {
      destroyed = true
      cancelAnimationFrame(frame)
      syncRef.current = () => {}
      resizeObserver.disconnect()
      intersection.disconnect()
      media.removeEventListener("change", sync)
      document.removeEventListener("visibilitychange", sync)
      if (gl) {
        if (buffer) gl.deleteBuffer(buffer)
        if (program) gl.deleteProgram(program)
        gl.getExtension("WEBGL_lose_context")?.loseContext()
      }
    }
  }, [])

  useEffect(() => {
    syncRef.current()
  }, [paused])

  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-0 size-full [filter:blur(1.25px)] [transform:scale(1.008)]",
        className,
      )}
    />
  )
}
