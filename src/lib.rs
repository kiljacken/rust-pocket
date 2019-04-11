use reqwest::Client;
use rustc_serialize::{json, Decodable, Decoder, Encodable, Encoder};
use std::convert::From;
use std::error::Error;
use std::io::Read;
use time::Timespec;
use url::Url;

pub trait JsonEncodable {
    fn json_encode(&self, e: &mut json::Encoder) -> Result<(), json::EncoderError>;
}

pub trait PocketAction: JsonEncodable {
    fn name(&self) -> &'static str;
}

impl<T: Encodable> JsonEncodable for T {
    fn json_encode(&self, e: &mut json::Encoder) -> Result<(), json::EncoderError> {
        Encodable::encode::<json::Encoder>(self, e)
    }
}

macro_rules! impl_item_pocket_action {
    ($name:expr, $cls:ident) => {
        pub struct $cls {
            item_id: u64,
            time: Option<u64>,
        }

        impl PocketAction for $cls {
            fn name(&self) -> &'static str {
                $name
            }
        }

        impl JsonEncodable for $cls {
            fn json_encode(&self, e: &mut json::Encoder) -> Result<(), json::EncoderError> {
                e.emit_struct(stringify!($cls), 3, |e| {
                    e.emit_struct_field("name", 0, |e| e.emit_str(self.name()))
                        .and_then(|_| {
                            e.emit_struct_field("item_id", 1, |e| e.emit_u64(self.item_id))
                        })
                        .and_then(|_| {
                            e.emit_struct_field("time", 2, |e| match self.time {
                                Some(v) => e.emit_option_some(|e| e.emit_u64(v)),
                                None => e.emit_option_none(),
                            })
                        })
                })
            }
        }
    };
}

#[derive(Debug)]
pub enum PocketError {
    Http(reqwest::Error),
    Io(std::io::Error),
    Json(json::DecoderError),
    Format(json::EncoderError),
    Proto(u16, String),
}

pub type PocketResult<T> = Result<T, PocketError>;

impl From<json::EncoderError> for PocketError {
    fn from(err: json::EncoderError) -> PocketError {
        PocketError::Format(err)
    }
}

impl From<json::DecoderError> for PocketError {
    fn from(err: json::DecoderError) -> PocketError {
        PocketError::Json(err)
    }
}

impl From<reqwest::Error> for PocketError {
    fn from(err: reqwest::Error) -> PocketError {
        PocketError::Http(err)
    }
}

impl From<std::io::Error> for PocketError {
    fn from(err: std::io::Error) -> PocketError {
        PocketError::Io(err)
    }
}

impl Error for PocketError {
    fn description(&self) -> &str {
        match *self {
            PocketError::Http(ref e) => e.description(),
            PocketError::Io(ref e) => e.description(),
            PocketError::Json(ref e) => e.description(),
            PocketError::Format(ref e) => e.description(),
            PocketError::Proto(..) => "protocol error",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match *self {
            PocketError::Http(ref e) => Some(e),
            PocketError::Io(ref e) => Some(e),
            PocketError::Json(ref e) => Some(e),
            PocketError::Format(ref e) => Some(e),
            PocketError::Proto(..) => None,
        }
    }
}

impl std::fmt::Display for PocketError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match *self {
            PocketError::Http(ref e) => e.fmt(fmt),
            PocketError::Io(ref e) => e.fmt(fmt),
            PocketError::Json(ref e) => e.fmt(fmt),
            PocketError::Format(ref e) => e.fmt(fmt),
            PocketError::Proto(ref code, ref msg) => {
                fmt.write_str(&*format!("{} (code {})", msg, code))
            }
        }
    }
}

pub struct Pocket {
    consumer_key: String,
    access_token: Option<String>,
    code: Option<String>,
    client: Client,
}

#[derive(RustcEncodable)]
pub struct PocketOAuthRequest<'a> {
    consumer_key: &'a str,
    redirect_uri: &'a str,
    state: Option<&'a str>,
}

#[derive(RustcDecodable)]
pub struct PocketOAuthResponse {
    code: String,
    state: Option<String>,
}

#[derive(RustcEncodable)]
pub struct PocketAuthorizeRequest<'a> {
    consumer_key: &'a str,
    code: &'a str,
}

#[derive(RustcDecodable)]
pub struct PocketAuthorizeResponse {
    access_token: String,
    username: String,
}

#[derive(RustcEncodable)]
pub struct PocketAddRequest<'a> {
    consumer_key: &'a str,
    access_token: &'a str,
    url: &'a str,
    title: Option<&'a str>,
    tags: Option<&'a str>,
    tweet_id: Option<&'a str>,
}

#[derive(RustcDecodable, Debug, PartialEq)]
pub struct ItemImage {
    pub item_id: u64,  // String
    pub image_id: u64, // String
    pub src: String,
    pub width: u16,  // String
    pub height: u16, // String
    pub caption: String,
    pub credit: String,
}

#[derive(Debug, PartialEq)]
pub struct ItemVideo {
    pub item_id: u64,  // String
    pub video_id: u64, // String
    pub src: String,
    pub width: u16,            // String
    pub height: u16,           // String
    pub length: Option<usize>, // String
    pub vid: String,
    pub vtype: u16,
}

impl Decodable for ItemVideo {
    fn decode<D: Decoder>(d: &mut D) -> Result<ItemVideo, D::Error> {
        d.read_struct("ItemVideo", 0, |d| {
            Ok(ItemVideo {
                item_id: d.read_struct_field("item_id", 0, |d| d.read_u64())?,
                video_id: d.read_struct_field("video_id", 1, |d| d.read_u64())?,
                src: d.read_struct_field("src", 2, Decodable::decode)?,
                width: d.read_struct_field("width", 3, |d| d.read_u16())?,
                height: d.read_struct_field("height", 4, |d| d.read_u16())?,
                length: d.read_struct_field("length", 5, |d| {
                    d.read_option(|d, b| {
                        if b {
                            d.read_usize().map(|v| Some(v))
                        } else {
                            Ok(None)
                        }
                    })
                })?,
                vid: d.read_struct_field("vid", 6, |d| d.read_str())?,
                vtype: d.read_struct_field("type", 7, |d| d.read_u16())?,
            })
        })
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PocketItemHas {
    No = 0,
    Yes = 1,
    Is = 2,
}

impl Decodable for PocketItemHas {
    fn decode<D: Decoder>(d: &mut D) -> Result<PocketItemHas, D::Error> {
        d.read_u8().map(|v| match v {
            0 => PocketItemHas::No,
            1 => PocketItemHas::Yes,
            2 => PocketItemHas::Is,
            _ => unreachable!(),
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct PocketAddedItem {
    pub item_id: u64,          // String
    pub extended_item_id: u64, // String

    pub given_url: String,
    pub normal_url: String,
    pub content_length: usize, // String
    pub word_count: usize,     // String
    pub encoding: String,
    pub mime_type: String, // must be Option<Mime>
    pub lang: String,
    pub title: String,
    pub excerpt: String,

    pub date_published: String, // must be Tm or Timespec
    pub date_resolved: String,  // must be Tm or Timespec

    pub resolved_id: u64, // String
    pub resolved_url: String,
    pub resolved_normal_url: String,

    pub login_required: bool, // String
    pub response_code: u16,
    pub used_fallback: bool, // String

    pub domain_id: u64,        // String
    pub origin_domain_id: u64, // String
    pub innerdomain_redirect: bool,

    pub is_index: bool,           // String
    pub is_article: bool,         // String
    pub has_image: PocketItemHas, // String
    pub has_video: PocketItemHas, // String

    //pub tags: Vec<ItemTag>, // ???
    //pub authors: Vec<ItemAuthor>, // ???
    pub videos: Vec<ItemVideo>, // encoded as object with integer indices
    pub images: Vec<ItemImage>, // if present, as empty array otherwise
}

impl Decodable for PocketAddedItem {
    fn decode<D: Decoder>(d: &mut D) -> Result<PocketAddedItem, D::Error> {
        d.read_struct("PocketAddedItem", 28, |d| {
            Ok(PocketAddedItem {
                item_id: d.read_struct_field("item_id", 0, |d| d.read_u64())?,
                extended_item_id: d.read_struct_field("extended_item_id", 1, |d| d.read_u64())?,

                given_url: d.read_struct_field("given_url", 2, Decodable::decode)?,
                normal_url: d.read_struct_field("normal_url", 3, Decodable::decode)?,
                content_length: d.read_struct_field("content_length", 4, |d| d.read_usize())?,
                word_count: d.read_struct_field("word_count", 5, |d| d.read_usize())?,
                encoding: d.read_struct_field("encoding", 6, |d| d.read_str())?,
                mime_type: d.read_struct_field("mime_type", 7, |d| d.read_str())?,
                lang: d.read_struct_field("lang", 8, |d| d.read_str())?,
                title: d.read_struct_field("title", 9, |d| d.read_str())?,
                excerpt: d.read_struct_field("excerpt", 10, |d| d.read_str())?,

                date_published: d.read_struct_field("date_published", 11, |d| d.read_str())?,
                date_resolved: d.read_struct_field("date_resolved", 12, |d| d.read_str())?,

                resolved_id: d.read_struct_field("resolved_id", 13, |d| d.read_u64())?,
                resolved_url: d.read_struct_field("resolved_url", 14, Decodable::decode)?,
                resolved_normal_url: d.read_struct_field(
                    "resolved_normal_url",
                    15,
                    Decodable::decode,
                )?,

                login_required: d
                    .read_struct_field("login_required", 16, |d| d.read_u8().map(|v| v != 0))?,
                response_code: d.read_struct_field("response_code", 17, |d| d.read_u16())?,
                used_fallback: d
                    .read_struct_field("used_fallback", 18, |d| d.read_u8().map(|v| v != 0))?,

                domain_id: d.read_struct_field("domain_id", 19, |d| d.read_u64())?,
                origin_domain_id: d.read_struct_field("origin_domain_id", 20, |d| d.read_u64())?,
                innerdomain_redirect: d.read_struct_field("innerdomain_redirect", 21, |d| {
                    d.read_u8().map(|v| v != 0)
                })?,

                is_index: d.read_struct_field("is_index", 22, |d| d.read_u8().map(|v| v != 0))?,
                is_article: d
                    .read_struct_field("is_article", 23, |d| d.read_u8().map(|v| v != 0))?,
                has_image: d.read_struct_field("has_image", 24, Decodable::decode)?,
                has_video: d.read_struct_field("has_video", 25, Decodable::decode)?,

                videos: d.read_struct_field("videos", 26, |d| {
                    d.read_seq(|d, s| {
                        Ok((0..s)
                            .flat_map(|i| d.read_seq_elt(i, Decodable::decode))
                            .into_iter()
                            .collect())
                    })
                })?,
                images: d.read_struct_field("images", 27, |d| {
                    d.read_seq(|d, s| {
                        Ok((0..s)
                            .flat_map(|i| d.read_seq_elt(i, Decodable::decode))
                            .into_iter()
                            .collect())
                    })
                })?,
            })
        })
    }
}

#[derive(RustcDecodable)]
pub struct PocketAddResponse {
    item: PocketAddedItem,
    status: u16,
}

pub struct PocketGetRequest<'a> {
    pocket: &'a mut Pocket,

    search: Option<&'a str>,
    domain: Option<&'a str>,

    tag: Option<PocketGetTag<'a>>,
    state: Option<PocketGetState>,
    content_type: Option<PocketGetType>,
    detail_type: Option<PocketGetDetail>,
    favorite: Option<bool>,
    since: Option<Timespec>,

    sort: Option<PocketGetSort>,
    count: Option<usize>,
    offset: Option<usize>,
}

impl<'a> Encodable for PocketGetRequest<'a> {
    fn encode<S: Encoder>(&self, e: &mut S) -> Result<(), S::Error> {
        e.emit_struct("PocketGetRequest", 13, |e| {
            e.emit_struct_field("consumer_key", 0, |e| self.pocket.consumer_key.encode(e))
                .and_then(|_| {
                    e.emit_struct_field("access_token", 1, |e| {
                        self.pocket.access_token.as_ref().unwrap().encode(e)
                    })
                })
                .and_then(|_| e.emit_struct_field("search", 2, |e| self.search.encode(e)))
                .and_then(|_| e.emit_struct_field("domain", 3, |e| self.domain.encode(e)))
                .and_then(|_| e.emit_struct_field("tag", 4, |e| self.tag.encode(e)))
                .and_then(|_| e.emit_struct_field("state", 5, |e| self.state.encode(e)))
                .and_then(|_| {
                    e.emit_struct_field("content_type", 6, |e| self.content_type.encode(e))
                })
                .and_then(|_| e.emit_struct_field("detail_type", 7, |e| self.detail_type.encode(e)))
                .and_then(|_| e.emit_struct_field("favorite", 8, |e| self.favorite.encode(e)))
                .and_then(|_| {
                    e.emit_struct_field("since", 9, |e| self.since.map(|v| v.sec).encode(e))
                })
                .and_then(|_| e.emit_struct_field("sort", 10, |e| self.sort.encode(e)))
                .and_then(|_| e.emit_struct_field("count", 11, |e| self.count.encode(e)))
                .and_then(|_| e.emit_struct_field("offset", 12, |e| self.offset.encode(e)))
        })
    }
}

impl<'a> PocketGetRequest<'a> {
    fn new(pocket: &'a mut Pocket) -> PocketGetRequest<'a> {
        PocketGetRequest {
            pocket: pocket,
            search: None,
            domain: None,
            tag: None,
            state: None,
            content_type: None,
            detail_type: None,
            favorite: None,
            since: None,
            sort: None,
            count: None,
            offset: None,
        }
    }

    pub fn search<'b>(&'b mut self, search: &'a str) -> &'b mut PocketGetRequest<'a> {
        self.search = Some(search);
        self
    }

    pub fn domain<'b>(&'b mut self, domain: &'a str) -> &'b mut PocketGetRequest<'a> {
        self.domain = Some(domain);
        self
    }

    pub fn tag<'b>(&'b mut self, tag: PocketGetTag<'a>) -> &'b mut PocketGetRequest<'a> {
        self.tag = Some(tag);
        self
    }

    pub fn state<'b>(&'b mut self, state: PocketGetState) -> &'b mut PocketGetRequest<'a> {
        self.state = Some(state);
        self
    }

    pub fn content_type<'b>(
        &'b mut self,
        content_type: PocketGetType,
    ) -> &'b mut PocketGetRequest<'a> {
        self.content_type = Some(content_type);
        self
    }

    pub fn detail_type<'b>(
        &'b mut self,
        detail_type: PocketGetDetail,
    ) -> &'b mut PocketGetRequest<'a> {
        self.detail_type = Some(detail_type);
        self
    }

    pub fn complete<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.detail_type(PocketGetDetail::Complete)
    }

    pub fn simple<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.detail_type(PocketGetDetail::Simple)
    }

    pub fn archived<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.state(PocketGetState::Archive)
    }

    pub fn unread<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.state(PocketGetState::Unread)
    }

    pub fn articles<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.content_type(PocketGetType::Article)
    }

    pub fn videos<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.content_type(PocketGetType::Video)
    }

    pub fn images<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.content_type(PocketGetType::Image)
    }

    pub fn favorite<'b>(&'b mut self, fav: bool) -> &'b mut PocketGetRequest<'a> {
        self.favorite = Some(fav);
        self
    }

    pub fn since<'b>(&'b mut self, since: Timespec) -> &'b mut PocketGetRequest<'a> {
        self.since = Some(since);
        self
    }

    pub fn sort<'b>(&'b mut self, sort: PocketGetSort) -> &'b mut PocketGetRequest<'a> {
        self.sort = Some(sort);
        self
    }

    pub fn sort_by_newest<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.sort(PocketGetSort::Newest)
    }

    pub fn sort_by_oldest<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.sort(PocketGetSort::Oldest)
    }

    pub fn sort_by_title<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.sort(PocketGetSort::Title)
    }

    pub fn sort_by_site<'b>(&'b mut self) -> &'b mut PocketGetRequest<'a> {
        self.sort(PocketGetSort::Site)
    }

    pub fn offset<'b>(&'b mut self, offset: usize) -> &'b mut PocketGetRequest<'a> {
        self.offset = Some(offset);
        self
    }

    pub fn count<'b>(&'b mut self, count: usize) -> &'b mut PocketGetRequest<'a> {
        self.count = Some(count);
        self
    }

    pub fn slice<'b>(&'b mut self, offset: usize, count: usize) -> &'b mut PocketGetRequest<'a> {
        self.offset(offset).count(count)
    }

    pub fn get(self) -> PocketResult<Vec<PocketItem>> {
        let mut request = String::new();
        {
            let mut encoder = json::Encoder::new(&mut request);
            self.encode(&mut encoder).unwrap();
        }

        self.pocket
            .request("https://getpocket.com/v3/get", &*request)
            .map(|v: PocketGetResponse| v.list)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PocketGetDetail {
    Simple,
    Complete,
}

impl Encodable for PocketGetDetail {
    fn encode<S: Encoder>(&self, e: &mut S) -> Result<(), S::Error> {
        e.emit_str(match *self {
            PocketGetDetail::Simple => "simple",
            PocketGetDetail::Complete => "complete",
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PocketGetSort {
    Newest,
    Oldest,
    Title,
    Site,
}

impl Encodable for PocketGetSort {
    fn encode<S: Encoder>(&self, e: &mut S) -> Result<(), S::Error> {
        e.emit_str(match *self {
            PocketGetSort::Newest => "newest",
            PocketGetSort::Oldest => "oldest",
            PocketGetSort::Title => "title",
            PocketGetSort::Site => "site",
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PocketGetState {
    Unread,
    Archive,
    All,
}

impl Encodable for PocketGetState {
    fn encode<S: Encoder>(&self, e: &mut S) -> Result<(), S::Error> {
        e.emit_str(match *self {
            PocketGetState::Unread => "unread",
            PocketGetState::Archive => "archive",
            PocketGetState::All => "all",
        })
    }
}

#[derive(Debug)]
pub enum PocketGetTag<'a> {
    Untagged,
    Tagged(&'a str),
}

impl<'a> Encodable for PocketGetTag<'a> {
    fn encode<S: Encoder>(&self, e: &mut S) -> Result<(), S::Error> {
        e.emit_str(match *self {
            PocketGetTag::Untagged => "_untagged_",
            PocketGetTag::Tagged(ref s) => s,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PocketGetType {
    Article,
    Video,
    Image,
}

impl Encodable for PocketGetType {
    fn encode<S: Encoder>(&self, e: &mut S) -> Result<(), S::Error> {
        e.emit_str(match *self {
            PocketGetType::Article => "article",
            PocketGetType::Video => "video",
            PocketGetType::Image => "image",
        })
    }
}

#[derive(Debug)]
pub struct PocketGetResponse {
    list: Vec<PocketItem>, // must be Vec
    status: u16,
    complete: bool, // must be bool
    error: Option<String>,
    //search_meta: PocketSearchMeta,
    since: Timespec,
}

impl Decodable for PocketGetResponse {
    fn decode<D: Decoder>(d: &mut D) -> Result<PocketGetResponse, D::Error> {
        d.read_struct("PocketGetResponse", 5, |d| {
            Ok(PocketGetResponse {
                list: d.read_struct_field("list", 0, |d| {
                    d.read_map(|d, s| {
                        Ok((0..s)
                            .flat_map(|i| {
                                d.read_map_elt_key(i, |d| d.read_str())
                                    .and_then(|_| d.read_map_elt_val(i, Decodable::decode))
                                    .into_iter()
                            })
                            .collect())
                    })
                })?,
                status: d.read_struct_field("status", 1, |d| d.read_u16())?,
                complete: d.read_struct_field("complete", 2, |d| d.read_u8().map(|v| v != 0))?,
                error: d.read_struct_field("error", 3, |d| {
                    d.read_option(|d, b| if b { d.read_str().map(Some) } else { Ok(None) })
                })?,
                since: d.read_struct_field("since", 4, |d| {
                    d.read_u64().map(|v| Timespec::new(v as i64, 0))
                })?,
            })
        })
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PocketItemStatus {
    Normal = 0,
    Archived = 1,
    Deleted = 2,
}

impl Decodable for PocketItemStatus {
    fn decode<D: Decoder>(d: &mut D) -> Result<PocketItemStatus, D::Error> {
        d.read_u8().map(|v| match v {
            0 => PocketItemStatus::Normal,
            1 => PocketItemStatus::Archived,
            2 => PocketItemStatus::Deleted,
            _ => unreachable!(),
        })
    }
}

// See also PocketAddedItem
#[derive(Debug, PartialEq)]
pub struct PocketItem {
    pub item_id: u64,

    pub given_url: String,
    pub given_title: String,

    pub word_count: usize,
    pub excerpt: String,

    pub time_added: Timespec,
    pub time_read: Timespec,
    pub time_updated: Timespec,
    pub time_favorited: Timespec,

    pub favorite: bool,

    pub is_index: bool,
    pub is_article: bool,
    pub has_image: PocketItemHas,
    pub has_video: PocketItemHas,

    pub resolved_id: u64,
    pub resolved_title: String,
    pub resolved_url: String,

    pub sort_id: usize,

    pub status: PocketItemStatus,
    pub images: Option<Vec<ItemImage>>,
    pub videos: Option<Vec<ItemVideo>>,
}

impl Decodable for PocketItem {
    fn decode<D: Decoder>(d: &mut D) -> Result<PocketItem, D::Error> {
        d.read_struct("PocketItem", 21, |d| {
            Ok(PocketItem {
                item_id: d.read_struct_field("item_id", 0, |d| d.read_u64())?,

                given_url: d.read_struct_field("given_url", 1, Decodable::decode)?,
                given_title: d.read_struct_field("given_title", 2, |d| d.read_str())?,

                word_count: d.read_struct_field("word_count", 3, |d| d.read_usize())?,
                excerpt: d.read_struct_field("excerpt", 4, |d| d.read_str())?,

                time_added: d.read_struct_field("time_added", 5, |d| {
                    d.read_u64().map(|v| Timespec::new(v as i64, 0))
                })?,
                time_read: d.read_struct_field("time_read", 6, |d| {
                    d.read_u64().map(|v| Timespec::new(v as i64, 0))
                })?,
                time_updated: d.read_struct_field("time_updated", 7, |d| {
                    d.read_u64().map(|v| Timespec::new(v as i64, 0))
                })?,
                time_favorited: d.read_struct_field("time_favorited", 8, |d| {
                    d.read_u64().map(|v| Timespec::new(v as i64, 0))
                })?,

                favorite: d.read_struct_field("favorite", 9, |d| d.read_u8().map(|v| v != 0))?,
                is_index: d.read_struct_field("is_index", 10, |d| d.read_u8().map(|v| v != 0))?,
                is_article: d
                    .read_struct_field("is_article", 11, |d| d.read_u8().map(|v| v != 0))?,
                has_image: d.read_struct_field("has_image", 12, Decodable::decode)?,
                has_video: d.read_struct_field("has_video", 13, Decodable::decode)?,

                resolved_id: d.read_struct_field("resolved_id", 14, |d| d.read_u64())?,
                resolved_title: d.read_struct_field("resolved_title", 15, |d| d.read_str())?,
                resolved_url: d.read_struct_field("resolved_url", 16, Decodable::decode)?,

                sort_id: d.read_struct_field("sort_id", 17, |d| d.read_usize())?,
                status: d.read_struct_field("status", 18, Decodable::decode)?,

                videos: d.read_struct_field("videos", 19, |d| {
                    d.read_option(|d, b| {
                        if b {
                            d.read_map(|d, s| {
                                Ok((0..s)
                                    .flat_map(|i| {
                                        d.read_map_elt_val(i, Decodable::decode).into_iter()
                                    })
                                    .collect())
                            })
                            .map(Some)
                        } else {
                            Ok(None)
                        }
                    })
                })?,
                images: d.read_struct_field("images", 20, |d| {
                    d.read_option(|d, b| {
                        if b {
                            d.read_map(|d, s| {
                                Ok((0..s)
                                    .flat_map(|i| {
                                        d.read_map_elt_val(i, Decodable::decode).into_iter()
                                    })
                                    .collect())
                            })
                            .map(Some)
                        } else {
                            Ok(None)
                        }
                    })
                })?,
            })
        })
    }
}

pub struct PocketAddAction<'a> {
    item_id: Option<u64>,
    ref_id: Option<&'a str>,
    tags: Option<&'a str>,
    time: Option<u64>,
    title: Option<&'a str>,
    url: Option<&'a str>,
}

impl<'a> PocketAction for PocketAddAction<'a> {
    fn name(&self) -> &'static str {
        "add"
    }
}

impl<'a> JsonEncodable for PocketAddAction<'a> {
    fn json_encode(&self, e: &mut json::Encoder) -> Result<(), json::EncoderError> {
        e.emit_struct("PocketAddAction", 7, |e| {
            e.emit_struct_field("name", 0, |e| e.emit_str(self.name()))
                .and_then(|_| e.emit_struct_field("item_id", 1, |e| self.item_id.encode(e)))
                .and_then(|_| e.emit_struct_field("ref_id", 2, |e| self.ref_id.encode(e)))
                .and_then(|_| e.emit_struct_field("tags", 3, |e| self.tags.encode(e)))
                .and_then(|_| e.emit_struct_field("time", 4, |e| self.time.encode(e)))
                .and_then(|_| e.emit_struct_field("title", 5, |e| self.title.encode(e)))
                .and_then(|_| e.emit_struct_field("url", 6, |e| self.url.encode(e)))
        })
    }
}

impl_item_pocket_action!("archive", PocketArchiveAction);
impl_item_pocket_action!("readd", PocketReaddAction);
impl_item_pocket_action!("favorite", PocketFavoriteAction);
impl_item_pocket_action!("unfavorite", PocketUnfavoriteAction);
impl_item_pocket_action!("delete", PocketDeleteAction);

pub struct PocketTagsAddAction<'a> {
    item_id: u64,
    tags: &'a str,
    time: Option<u64>,
}

impl<'a> PocketAction for PocketTagsAddAction<'a> {
    fn name(&self) -> &'static str {
        "tags_add"
    }
}

impl<'a> JsonEncodable for PocketTagsAddAction<'a> {
    fn json_encode(&self, e: &mut json::Encoder) -> Result<(), json::EncoderError> {
        e.emit_struct("PocketTagsAddAction", 3, |e| {
            e.emit_struct_field("name", 0, |e| e.emit_str(self.name()))
                .and_then(|_| e.emit_struct_field("tags", 1, |e| self.tags.encode(e)))
                .and_then(|_| e.emit_struct_field("time", 2, |e| self.time.encode(e)))
        })
    }
}

pub struct PocketTagsReplaceAction<'a> {
    item_id: u64,
    tags: &'a str,
    time: Option<u64>,
}

impl<'a> PocketAction for PocketTagsReplaceAction<'a> {
    fn name(&self) -> &'static str {
        "tags_replace"
    }
}

impl<'a> JsonEncodable for PocketTagsReplaceAction<'a> {
    fn json_encode(&self, e: &mut json::Encoder) -> Result<(), json::EncoderError> {
        e.emit_struct("PocketTagsReplaceAction", 4, |e| {
            e.emit_struct_field("name", 0, |e| e.emit_str(self.name()))
                .and_then(|_| e.emit_struct_field("item_id", 1, |e| self.item_id.encode(e)))
                .and_then(|_| e.emit_struct_field("tags", 2, |e| self.tags.encode(e)))
                .and_then(|_| e.emit_struct_field("time", 3, |e| self.time.encode(e)))
        })
    }
}

impl_item_pocket_action!("tags_clear", PocketTagsClearAction);

pub struct PocketTagRenameAction<'a> {
    item_id: u64,
    old_tag: &'a str,
    new_tag: &'a str,
    time: Option<u64>,
}

impl<'a> PocketAction for PocketTagRenameAction<'a> {
    fn name(&self) -> &'static str {
        "tag_rename"
    }
}

impl<'a> JsonEncodable for PocketTagRenameAction<'a> {
    fn json_encode(&self, e: &mut json::Encoder) -> Result<(), json::EncoderError> {
        e.emit_struct("PocketTagRenameAction", 5, |e| {
            e.emit_struct_field("name", 0, |e| e.emit_str(self.name()))
                .and_then(|_| e.emit_struct_field("item_id", 1, |e| self.item_id.encode(e)))
                .and_then(|_| e.emit_struct_field("old_tag", 2, |e| self.old_tag.encode(e)))
                .and_then(|_| e.emit_struct_field("new_tag", 3, |e| self.new_tag.encode(e)))
                .and_then(|_| e.emit_struct_field("time", 4, |e| self.time.encode(e)))
        })
    }
}

pub struct PocketSendRequest<'a, 'b> {
    pocket: &'b mut Pocket,
    actions: &'a [&'a PocketAction],
}

impl<'a, 'b> JsonEncodable for PocketSendRequest<'a, 'b> {
    fn json_encode(&self, e: &mut json::Encoder) -> Result<(), json::EncoderError> {
        e.emit_struct("PocketSendRequest", 3, |e| {
            e.emit_struct_field("consumer_key", 0, |e| self.pocket.consumer_key.encode(e))
                .and_then(|_| {
                    e.emit_struct_field("access_token", 1, |e| {
                        self.pocket.access_token.as_ref().unwrap().encode(e)
                    })
                })
                .and_then(|_| {
                    e.emit_struct_field("actions", 2, |e| {
                        e.emit_seq(self.actions.len(), |e| {
                            for (i, action) in self.actions.iter().enumerate() {
                                e.emit_seq_elt(i, |e| action.json_encode(e))?;
                            }
                            Ok(())
                        })
                    })
                })
        })
    }
}

#[derive(RustcDecodable)]
pub struct PocketSendResponse {
    status: u16,
    action_results: Vec<bool>,
}

impl Pocket {
    pub fn new(consumer_key: &str, access_token: Option<&str>) -> Pocket {
        Pocket {
            consumer_key: consumer_key.to_string(),
            access_token: access_token.map(|v| v.to_string()),
            code: None,
            client: Client::new(),
        }
    }

    #[inline]
    pub fn access_token(&self) -> Option<&str> {
        self.access_token.as_ref().map(|v| &**v)
    }

    fn request<Resp: Decodable>(&mut self, url: &str, data: &str) -> PocketResult<Resp> {
        self.client
            .post(url)
            .header("XAccept", "application/json")
            .header("ContentType", "application/json")
            .body(data.to_string())
            .send()
            .map_err(From::from)
            .and_then(|mut r| match r.headers().get("XErrorCode") {
                None => {
                    let mut out = String::new();
                    r.read_to_string(&mut out).map_err(From::from).map(|_| out)
                }
                Some(code) => {
                    let code = code.to_str().unwrap().parse().unwrap();
                    let error = r
                        .headers()
                        .get("XError")
                        .map(|v| v.to_str().unwrap())
                        .unwrap_or("unknown protocol error")
                        .to_string();
                    Err(PocketError::Proto(code, error))
                }
            })
            .and_then(|s| json::decode::<Resp>(&*s).map_err(From::from))
    }

    pub fn get_auth_url(&mut self) -> PocketResult<Url> {
        let request = json::encode(&PocketOAuthRequest {
            consumer_key: &*self.consumer_key,
            redirect_uri: "rustapi:finishauth",
            state: None,
        })?;

        self.request("https://getpocket.com/v3/oauth/request", &*request)
            .and_then(|r: PocketOAuthResponse| {
                let mut url = Url::parse("https://getpocket.com/auth/authorize").unwrap();
                url.set_query_from_pairs(
                    vec![
                        ("request_token", &*r.code),
                        ("redirect_uri", "rustapi:finishauth"),
                    ]
                    .into_iter(),
                );
                self.code = Some(r.code);
                Ok(url)
            })
    }

    pub fn authorize(&mut self) -> PocketResult<String> {
        let request = json::encode(&PocketAuthorizeRequest {
            consumer_key: &*self.consumer_key,
            code: self.code.as_ref().map(|v| &**v).unwrap(),
        })?;

        match self.request("https://getpocket.com/v3/oauth/authorize", &*request) {
            Ok(r @ PocketAuthorizeResponse { .. }) => {
                self.access_token = Some(r.access_token);
                Ok(r.username)
            }
            Err(e) => Err(e),
        }
    }

    pub fn add(
        &mut self,
        url: &str,
        title: Option<&str>,
        tags: Option<&str>,
        tweet_id: Option<&str>,
    ) -> PocketResult<PocketAddedItem> {
        let request = json::encode(&PocketAddRequest {
            consumer_key: &*self.consumer_key,
            access_token: &**self.access_token.as_ref().unwrap(),
            url: url,
            title: title.map(|v| v.clone()),
            tags: tags.map(|v| v.clone()),
            tweet_id: tweet_id.map(|v| v.clone()),
        })?;

        self.request("https://getpocket.com/v3/add", &*request)
            .map(|v: PocketAddResponse| v.item)
    }

    #[inline]
    pub fn push(&mut self, url: &str) -> PocketResult<PocketAddedItem> {
        self.add(url, None, None, None)
    }

    pub fn filter(&mut self) -> PocketGetRequest {
        PocketGetRequest::new(self)
    }
}

#[test]
fn test_actions_serialize() {
    let mut pocket = Pocket::new("abc", Some("def"));
    let add_action = PocketAddAction {
        item_id: None,
        ref_id: None,
        tags: None,
        time: None,
        title: None,
        url: None,
    };
    let act: &PocketAction = &add_action;
    let actions = PocketSendRequest {
        pocket: &mut pocket,
        actions: &[act],
    };
    //assert_eq!(&*actions.to_json().to_string(), "{

    //}");
}
