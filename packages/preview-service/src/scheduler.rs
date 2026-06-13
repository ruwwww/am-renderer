use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

use graph_resolver::model::Project;
use renderer_core::compositor::ImageCache;

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IncomingMessage {
    Seek { frame: u32 },
    Play { fps: Option<f32> },
    Pause,
    Config { scale: f32 },
}

#[derive(Clone, Debug)]
pub struct FramePackage {
    pub frame: u32,
    pub data: Vec<u8>,
}

pub struct SchedulerState {
    pub current_frame: u32,
    pub is_playing: bool,
    pub fps: f32,
    pub proxy_scale: f32,
    pub active_renders: Vec<JoinHandle<()>>,
}

pub struct PlaybackScheduler {
    pub project: Arc<Project>,
    pub assets_dir: PathBuf,
    pub state: Arc<Mutex<SchedulerState>>,
    pub image_cache: Arc<Mutex<ImageCache>>,
    pub frame_sender: mpsc::Sender<FramePackage>,
    pub tick_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl PlaybackScheduler {
    pub fn new(
        project: Project,
        assets_dir: PathBuf,
        frame_sender: mpsc::Sender<FramePackage>,
    ) -> Self {
        let fps = project.fps;
        Self {
            project: Arc::new(project),
            assets_dir,
            state: Arc::new(Mutex::new(SchedulerState {
                current_frame: 0,
                is_playing: false,
                fps,
                proxy_scale: 0.5, // 540p / half-resolution by default for speed
                active_renders: Vec::new(),
            })),
            image_cache: Arc::new(Mutex::new(ImageCache::new())),
            frame_sender,
            tick_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Update the scaling factor for preview renders
    pub async fn set_scale(&self, scale: f32) {
        let mut state = self.state.lock().await;
        state.proxy_scale = scale.clamp(0.05, 1.0);
        let current = state.current_frame;
        drop(state);
        self.trigger_render(current).await;
    }

    /// Trigger a seek to a specific frame
    pub async fn seek(&self, frame: u32) {
        let target_frame = frame.min(self.project.total_frames().saturating_sub(1));
        
        let mut state = self.state.lock().await;
        state.current_frame = target_frame;
        
        // Cancel all ongoing renders
        for handle in state.active_renders.drain(..) {
            handle.abort();
        }
        drop(state);

        self.trigger_render(target_frame).await;
        self.prefetch_upcoming(target_frame).await;
    }

    /// Start the active playback ticker
    pub async fn play(&self, target_fps: Option<f32>) {
        let mut state = self.state.lock().await;
        if state.is_playing {
            return;
        }
        state.is_playing = true;
        if let Some(fps) = target_fps {
            state.fps = fps;
        }
        let fps = state.fps;
        drop(state);

        let scheduler_state = self.state.clone();
        let tick_rate = Duration::from_secs_f32(1.0 / fps);
        let scheduler_weak = Arc::downgrade(&Arc::new(self.clone_scheduler())); // Avoid cycles

        let handle = tokio::spawn(async move {
            let mut ticker = interval(tick_rate);
            // Skip the first immediate tick
            ticker.tick().await;

            loop {
                ticker.tick().await;
                
                let mut state = scheduler_state.lock().await;
                if !state.is_playing {
                    break;
                }

                // Tick playhead
                state.current_frame += 1;
                let frame = state.current_frame;
                
                // If we hit the end, loop or pause
                if let Some(sched) = scheduler_weak.upgrade() {
                    if frame >= sched.project.total_frames() {
                        state.current_frame = 0;
                        let loop_frame = state.current_frame;
                        drop(state);
                        sched.trigger_render(loop_frame).await;
                    } else {
                        drop(state);
                        sched.trigger_render(frame).await;
                        // Prefetch ahead
                        sched.prefetch_upcoming(frame).await;
                    }
                } else {
                    break;
                }
            }
        });

        let mut handle_store = self.tick_handle.lock().await;
        if let Some(old) = handle_store.replace(handle) {
            old.abort();
        }
    }

    /// Pause the playback ticker
    pub async fn pause(&self) {
        let mut state = self.state.lock().await;
        state.is_playing = false;
        drop(state);

        let mut handle_store = self.tick_handle.lock().await;
        if let Some(handle) = handle_store.take() {
            handle.abort();
        }
    }

    /// Clone minimal pointers to recreate the scheduler interface inside spawn tasks
    pub(crate) fn clone_scheduler(&self) -> Self {
        Self {
            project: self.project.clone(),
            assets_dir: self.assets_dir.clone(),
            state: self.state.clone(),
            image_cache: self.image_cache.clone(),
            frame_sender: self.frame_sender.clone(),
            tick_handle: self.tick_handle.clone(),
        }
    }

    /// Pre-render upcoming frames to buffer playback
    async fn prefetch_upcoming(&self, start_frame: u32) {
        let state = self.state.lock().await;
        let scale = state.proxy_scale;
        let is_playing = state.is_playing;
        drop(state);

        // Prefetch buffer sizes: larger if playing
        let buffer_size = if is_playing { 10 } else { 3 };
        let total_frames = self.project.total_frames();

        for i in 1..=buffer_size {
            let f = start_frame + i;
            if f >= total_frames {
                break;
            }

            let project = self.project.clone();
            let assets_dir = self.assets_dir.clone();
            let cache_lock = self.image_cache.clone();
            let sender = self.frame_sender.clone();

            let render_task = tokio::spawn(async move {
                let time_secs = f as f32 / project.fps;
                let resolved = graph_resolver::eval::timeline::evaluate(&project, time_secs);
                let mut cache = cache_lock.lock().await;

                if let Ok(img) = renderer_core::compositor::render_scene(
                    &resolved,
                    &mut cache,
                    &assets_dir,
                    false,
                    &[],
                ) {
                    let final_img = if (scale - 1.0).abs() > 0.01 && scale > 0.0 {
                        let w = (img.width() as f32 * scale).round() as u32;
                        let h = (img.height() as f32 * scale).round() as u32;
                        image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle)
                    } else {
                        img
                    };

                    let webp_bytes = compress_to_webp(&final_img, 70.0);
                    let _ = sender.send(FramePackage {
                        frame: f,
                        data: webp_bytes,
                    }).await;
                }
            });

            // Lock again to record task handle
            let mut state = self.state.lock().await;
            state.active_renders.push(render_task);
        }
    }

    /// Trigger evaluation, rendering, and WebP compression of a single frame
    async fn trigger_render(&self, frame: u32) {
        let project = self.project.clone();
        let assets_dir = self.assets_dir.clone();
        let cache_lock = self.image_cache.clone();
        let sender = self.frame_sender.clone();
        
        let state = self.state.lock().await;
        let scale = state.proxy_scale;
        drop(state);

        let render_task = tokio::spawn(async move {
            let time_secs = frame as f32 / project.fps;
            let resolved = graph_resolver::eval::timeline::evaluate(&project, time_secs);
            let mut cache = cache_lock.lock().await;

            if let Ok(img) = renderer_core::compositor::render_scene(
                &resolved,
                &mut cache,
                &assets_dir,
                false,
                &[],
            ) {
                let final_img = if (scale - 1.0).abs() > 0.01 && scale > 0.0 {
                    let w = (img.width() as f32 * scale).round() as u32;
                    let h = (img.height() as f32 * scale).round() as u32;
                    image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle)
                } else {
                    img
                };

                let webp_bytes = compress_to_webp(&final_img, 70.0);
                let _ = sender.send(FramePackage {
                    frame,
                    data: webp_bytes,
                }).await;
            }
        });

        let mut state = self.state.lock().await;
        state.active_renders.push(render_task);
    }
}

fn compress_to_webp(img: &image::RgbaImage, quality: f32) -> Vec<u8> {
    let encoder = webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height());
    let memory = encoder.encode(quality);
    memory.to_vec()
}
