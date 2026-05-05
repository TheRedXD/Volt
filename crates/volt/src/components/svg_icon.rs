use gpui::*;
use gpui::{
    App, Context, Entity, Global, ImageCacheError, ImageSource, 
    IntoElement, RenderImage, RenderOnce, SharedString, Window,
    div, img, px, Styled,
};
use std::collections::{HashSet};
use image::{Rgba, ImageBuffer};
use resvg::{tiny_skia, usvg};
use std::{collections::HashMap, sync::Arc};

struct SvgAssetCache {
    images: HashMap<(String, u32, u32), ImageSource>,
}

impl Global for SvgAssetCache {}

pub fn render_svg_to_pixels(svg_data: &str, width: u32, height: u32) -> Vec<u8> {
    let tree = usvg::Tree::from_str(svg_data, &usvg::Options::default())
        .expect("Failed to parse SVG");

    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .expect("Failed to create pixmap");

    let svg_size = tree.size();
    let scale_x = width as f32 / svg_size.width();
    let scale_y = height as f32 / svg_size.height();
    let scale = scale_x.min(scale_y);

    let dx = (width as f32 - (svg_size.width() * scale)) / 2.0;
    let dy = (height as f32 - (svg_size.height() * scale)) / 2.0;

    let transform = tiny_skia::Transform::from_scale(scale, scale)
        .post_translate(dx, dy);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    pixmap.data().to_vec()
}

struct GlobalIconRegistry(Entity<IconRegistry>);
impl Global for GlobalIconRegistry {}

pub struct IconRegistry {
    cache: HashMap<(SharedString, u32, u32), ImageSource>,
    loading: HashSet<(SharedString, u32, u32)>,
}

impl IconRegistry {
    pub fn global(cx: &mut App) -> Entity<Self> {
        if cx.has_global::<GlobalIconRegistry>() {
            cx.global::<GlobalIconRegistry>().0.clone()
        } else {
            let entity = cx.new(|_| IconRegistry {
                cache: HashMap::new(),
                loading: HashSet::new(),
            });
            cx.set_global(GlobalIconRegistry(entity.clone()));
            entity
        }
    }

    pub fn get(
        &mut self,
        path: impl Into<SharedString>,
        width: u32,
        height: u32,
        cx: &mut Context<Self>,
    ) -> Option<ImageSource> {
        let path = path.into();
        let key = (path.clone(), width, height);

        if let Some(source) = self.cache.get(&key) {
            return Some(source.clone());
        }

        if self.loading.insert(key.clone()) {
            let asset_source = cx.asset_source().clone();
            let mut async_cx = cx.to_async();

            cx.spawn(move |this: WeakEntity<IconRegistry>, _: &mut _| {
                let fut = async move {
                    let asset_bytes = async_cx
                        .background_executor()
                        .spawn(async move {
                            asset_source
                                .load(path.as_ref())
                                .unwrap()
                                .expect("SVG Asset not found in AssetProvider")
                        })
                        .await;

                    let svg_str = std::str::from_utf8(asset_bytes.as_ref())
                        .expect("SVG is not valid UTF-8")
                        .to_owned();

                    let mut pixels: Vec<u8> = async_cx
                        .background_executor()
                        .spawn(async move { render_svg_to_pixels(&svg_str, width, height) })
                        .await;

                    let _ = this.update(&mut async_cx, |this, cx| {
                        for px in pixels.chunks_exact_mut(4) {
                            px.swap(0, 2);
                        }

                        let image_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, pixels)
                            .expect("Failed to construct ImageBuffer");

                        let render_image = Arc::new(RenderImage::new(
                            smallvec::smallvec![image::Frame::new(image_buffer)]
                        ));

                        let image_source = ImageSource::from({
                            let img_clone = render_image.clone();
                            move |_window: &mut Window, _cx: &mut App| Some(Ok::<_, ImageCacheError>(img_clone.clone()))
                        });

                        this.cache.insert(key.clone(), image_source);
                        this.loading.remove(&key);
                        
                        cx.notify(); 
                    });
                };
                fut
            }).detach();
        }

        None
    }
}

#[derive(IntoElement)]
pub struct SvgIcon {
    path: SharedString,
    width: u32,
    height: u32,
}

impl SvgIcon {
    pub fn new(path: impl Into<SharedString>, width: u32, height: u32) -> Self {
        Self { path: path.into(), width, height }
    }
}

impl RenderOnce for SvgIcon {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let registry_entity = IconRegistry::global(cx);
        
        let image_source = registry_entity.update(cx, |registry, cx| {
            registry.get(self.path, self.width, self.height, cx)
        });

        if let Some(source) = image_source {
            div().child(
                img(source).w(px(self.width as f32)).h(px(self.height as f32))
            )
        } else {
            div().w(px(self.width as f32)).h(px(self.height as f32))
        }
    }
}