use super::blocks::DescBlock;
use super::home::Story;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CachedStory {
    pub title: String,
    pub banner_url: String,
    pub body_html: String,
    pub blocks: Option<Vec<DescBlock>>,
}

fn key(id: &str) -> String {
    format!("{}:{id}", super::forge::user_lang())
}

pub fn get(id: &str) -> Option<CachedStory> {
    crate::cache::stories().get(&key(id))
}

pub fn remember(story: &Story) {
    let blocks = crate::cache::stories()
        .get(&key(&story.id))
        .filter(|c| c.body_html == story.body_html)
        .and_then(|c| c.blocks);

    crate::cache::stories().insert(
        key(&story.id),
        CachedStory {
            title: story.title.clone(),
            banner_url: story.banner_url.clone(),
            body_html: story.body_html.clone(),
            blocks,
        },
    );
}

pub fn remember_all(stories: &[Story]) {
    for story in stories {
        remember(story);
    }
}

pub fn seed(stories: &[Story]) {
    for story in stories {
        if !crate::cache::stories().contains_key(&key(&story.id)) {
            remember(story);
        }
    }
}

pub fn store_blocks(id: &str, blocks: Vec<DescBlock>) {
    crate::cache::stories().update(&key(id), |c| c.blocks = Some(blocks));
}
