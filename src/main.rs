#![warn(trivial_casts)]
#![deny(unused)]
#![forbid(unused_import_braces)]

mod user_list;

use std::{
    collections::{
        HashMap,
        HashSet
    },
    env,
    fmt,
    fs::File,
    io::{
        self,
        BufReader,
        prelude::*
    },
    iter,
    net::{
        TcpListener,
        TcpStream
    },
    process::{
        Command,
        Stdio
    },
    str::FromStr,
    sync::{
        Arc,
        PoisonError,
        RwLockReadGuard
    },
    thread,
    time::Duration
};
use chrono::prelude::*;
use kuchiki::traits::TendrilSink;
use mtg::{
    card::{
        Card,
        Db
    },
    color::ColorSet
};
use rand::prelude::*;
use serde_derive::Deserialize;
use serenity::{
    builder::CreateEmbed,
    client::bridge::gateway::ShardManager,
    http::raw::get_webhook_with_token,
    model::prelude::*,
    prelude::*,
    utils::MessageBuilder
};
use typemap::{
    Key,
    ShareMap
};
use url::Url;

macro_rules! manamoji {
    ($($sym:expr => $name:expr, $id:expr;)+) => {
        #[allow(unused)] //TODO
        fn manamoji(s: &str) -> Option<ReactionType> {
            match s {
                $($sym => Some(ReactionType::Custom {
                    id: EmojiId($id),
                    name: Some($name.to_owned()),
                    animated: false
                }),)+
                _ => None
            }
        }

        fn with_manamoji<S: ToString>(s: S) -> String {
            let s = s.to_string();
            $(
                let mut split = s.split($sym);
                let mut s = split.next().expect("failed to convert manamoji").to_owned();
                for part in split {
                    s.push_str(&ReactionType::Custom {
                        id: EmojiId($id),
                        name: Some($name.to_owned()),
                        animated: false
                    }.to_string());
                    s.push_str(part);
                }
            )+
            s
        }
    };
}

manamoji! {
    "{0}" => "mana0", 386740600436686878;
    "{1}" => "mana1", 386740600399069204;
    "{10}" => "mana10", 386740603708506142;
    "{11}" => "mana11", 386741116927475718;
    "{12}" => "mana12", 386741117577592833;
    "{13}" => "mana13", 386741118303469570;
    "{14}" => "mana14", 386741118668242964;
    "{15}" => "mana15", 386741118273847307;
    "{16}" => "mana16", 386741118597070850;
    "{17}" => "mana17", 386741120434044948;
    "{18}" => "mana18", 386741120006094860;
    "{19}" => "mana19", 386741120371261449;
    "{2}" => "mana2", 386740600755453953;
    "{2/B}" => "mana2b", 386741115807596547;
    "{2/G}" => "mana2g", 386741116118106122;
    "{2/R}" => "mana2r", 386741116222963712;
    "{2/U}" => "mana2u", 386741115933687809;
    "{2/W}" => "mana2w", 386741116462039043;
    "{20}" => "mana20", 386741120543227906;
    "{3}" => "mana3", 386740601082871822;
    "{4}" => "mana4", 386740601556828160;
    "{5}" => "mana5", 386740601724338176;
    "{6}" => "mana6", 386740602265403392;
    "{7}" => "mana7", 386740602374717440;
    "{8}" => "mana8", 386740602747879435;
    "{9}" => "mana9", 386740603494334484;
    "{A}" => "manarunic", 483811996924379156;
    "{B}" => "manab", 386740604341583873;
    "{B/G}" => "manabg", 386740605847470081;
    "{B/P}" => "manabp", 386741121486815232;
    "{B/R}" => "manabr", 386740607017549838;
    "{C}" => "manac", 386740607416139777;
    "{CHAOS}" => "manachaos", 386741121608318986;
    "{E}" => "manae", 386741121780416513;
    "{G}" => "manag", 386740607827181569;
    "{G/P}" => "managp", 386741122225012761;
    "{G/U}" => "managu", 386740607906873344;
    "{G/W}" => "managw", 386740607994953728;
    "{P}" => "manap", 483816536176590898;
    "{Q}" => "manaq", 386741125987434506;
    "{R}" => "manar", 386740612394778634;
    "{R/G}" => "manarg", 386740613430640640;
    "{R/P}" => "manarp", 386741125773262860;
    "{R/W}" => "manarw", 386740615498563588;
    "{S}" => "manas", 386741126939541505;
    "{T}" => "manat", 386740612143120385;
    "{U}" => "manau", 386740612289789953;
    "{U/B}" => "manaub", 386740615683112992;
    "{U/P}" => "manaup", 386741126247350284;
    "{U/R}" => "manaur", 386740615649558534;
    "{W}" => "manaw", 386740617792978944;
    "{W/B}" => "manawb", 386740617780264960;
    "{W/P}" => "manawp", 386741126645809162;
    "{W/U}" => "manawu", 386740617792978964;
    "{X}" => "manax", 386740617667018752;
    "{Y}" => "manay", 386741126457065473;
    "{Z}" => "manaz", 386741127207845890;
    "{hr}" => "manahr", 386741125672730634;
    "{hw}" => "manahw", 386741125773262848;
    "{½}" => "manahalf", 386741122510225421;
    "{∞}" => "manainfinity", 386741125861605387;
}

macro_rules! indicator_emoji {
    ($($colors:pat => $name:expr, $id:expr;)+) => {
        fn indicator_emoji(colors: &ColorSet) -> Emoji {
            match *colors {
                $($colors => Emoji {
                    animated: false,
                    id: EmojiId($id),
                    name: $name.to_owned(),
                    managed: false,
                    require_colons: true,
                    roles: Vec::default()
                },)+
            }
        }
    };
}
indicator_emoji! {
    ColorSet { white: false, blue: false, black: false, red: false, green: false } => "in_c", 493537120628244481;
    ColorSet { white: true, blue: false, black: false, red: false, green: false } => "in_w", 493480100252352512;
    ColorSet { white: false, blue: true, black: false, red: false, green: false } => "in_u", 493480399532589067;
    ColorSet { white: false, blue: false, black: true, red: false, green: false } => "in_b", 493480399146844171;
    ColorSet { white: false, blue: false, black: false, red: true, green: false } => "in_r", 493480399545040910;
    ColorSet { white: false, blue: false, black: false, red: false, green: true } => "in_g", 493480399737978883;
    ColorSet { white: true, blue: true, black: false, red: false, green: false } => "in_wu", 493518454775873536;
    ColorSet { white: false, blue: true, black: true, red: false, green: false } => "in_ub", 493518454033219585;
    ColorSet { white: false, blue: false, black: true, red: true, green: false } => "in_br", 493518453249015809;
    ColorSet { white: false, blue: false, black: false, red: true, green: true } => "in_rg", 493518453886550016;
    ColorSet { white: true, blue: false, black: false, red: false, green: true } => "in_gw", 493518453919973407;
    ColorSet { white: true, blue: false, black: true, red: false, green: false } => "in_wb", 493518454108979202;
    ColorSet { white: false, blue: true, black: false, red: true, green: false } => "in_ur", 493518454268100650;
    ColorSet { white: false, blue: false, black: true, red: false, green: true } => "in_bg", 493518453123186689;
    ColorSet { white: true, blue: false, black: false, red: true, green: false } => "in_rw", 493518453999927307;
    ColorSet { white: false, blue: true, black: false, red: false, green: true } => "in_gu", 493518453249015810;
    ColorSet { white: true, blue: true, black: false, red: false, green: true } => "in_gwu", 493531781744689153;
    ColorSet { white: true, blue: true, black: true, red: false, green: false } => "in_wub", 493531782210125864;
    ColorSet { white: false, blue: true, black: true, red: true, green: false } => "in_ubr", 493531782508052520;
    ColorSet { white: false, blue: false, black: true, red: true, green: true } => "in_brg", 493531781799084032;
    ColorSet { white: true, blue: false, black: false, red: true, green: true } => "in_rgw", 493531781660672005;
    ColorSet { white: true, blue: false, black: true, red: false, green: true } => "in_wbg", 493531782386548736;
    ColorSet { white: true, blue: true, black: false, red: true, green: false } => "in_urw", 493531782130696193;
    ColorSet { white: false, blue: true, black: true, red: false, green: true } => "in_bgu", 493531780666884097;
    ColorSet { white: true, blue: false, black: true, red: true, green: false } => "in_rwb", 493531782352732160;
    ColorSet { white: false, blue: true, black: false, red: true, green: true } => "in_gur", 493531781782306863;
    ColorSet { white: true, blue: true, black: true, red: true, green: false } => "in_wubr", 493536592296673290;
    ColorSet { white: false, blue: true, black: true, red: true, green: true } => "in_ubrg", 493536591973974046;
    ColorSet { white: true, blue: false, black: true, red: true, green: true } => "in_brgw", 493536591403286563;
    ColorSet { white: true, blue: true, black: false, red: true, green: true } => "in_rgwu", 493536591482978315;
    ColorSet { white: true, blue: true, black: true, red: false, green: true } => "in_gwub", 493536591474589699;
    ColorSet { white: true, blue: true, black: true, red: true, green: true } => "in_wubrg", 493540446623236096;
}

const HOSTNAME: &str = "lore-seeker.cards";
const IPC_ADDR: &str = "127.0.0.1:18806";

struct CardDb;

impl Key for CardDb {
    type Value = Db;
}

struct InlineChannels;

impl Key for InlineChannels {
    type Value = HashSet<ChannelId>;
}

struct InlineGuilds;

impl Key for InlineGuilds {
    type Value = HashSet<GuildId>;
}

struct Owners;

impl Key for Owners {
    type Value = HashSet<UserId>;
}

struct ShardManagerContainer;

impl Key for ShardManagerContainer {
    type Value = Arc<Mutex<ShardManager>>;
}

#[derive(Deserialize)]
struct WebhookData {
    id: WebhookId,
    token: String
}

impl Key for WebhookData {
    type Value = HashMap<ChannelId, Webhook>;
}

impl WebhookData {
    fn to_webhook(&self) -> serenity::Result<Webhook> {
        get_webhook_with_token(self.id.0, &self.token)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    #[serde(default)]
    inline_channels: HashSet<ChannelId>,
    #[serde(default)]
    inline_guilds: HashSet<GuildId>,
    bot_token: String,
    #[serde(default)]
    webhooks: HashMap<ChannelId, WebhookData>
}

#[derive(Default)]
struct Handler(Arc<Mutex<Option<Context>>>);

impl EventHandler for Handler {
    fn ready(&self, ctx: Context, ready: Ready) {
        *self.0.lock() = Some(ctx.clone());
        let guilds = ready.user.guilds().expect("failed to get guilds");
        if guilds.is_empty() {
            println!("[!!!!] No guilds found, use following URL to invite the bot:");
            println!("[ ** ] {}", ready.user.invite_url(Permissions::READ_MESSAGES | Permissions::SEND_MESSAGES | Permissions::USE_EXTERNAL_EMOJIS).expect("failed to generate invite URL"));
            shut_down(&ctx);
        }
    }

    fn guild_ban_addition(&self, _: Context, guild_id: GuildId, user: User) {
        user_list::remove(guild_id, user).expect("failed to remove banned user from user list");
    }

    fn guild_ban_removal(&self, _: Context, guild_id: GuildId, user: User) {
        user_list::add(guild_id, guild_id.member(user).expect("failed to get unbanned guild member")).expect("failed to add unbanned user to user list");
    }

    fn guild_create(&self, _: Context, guild: Guild, _: bool) {
        user_list::set_guild(guild.id, guild.members.values().cloned()).expect("failed to initialize user list");
        println!("[ ** ] Connected to {}", guild.name);
    }

    fn guild_member_addition(&self, _: Context, guild_id: GuildId, member: Member) {
        user_list::add(guild_id, member).expect("failed to add new guild member to user list");
    }

    fn guild_member_removal(&self, _: Context, guild_id: GuildId, user: User, _: Option<Member>) {
        user_list::remove(guild_id, user).expect("failed to remove removed guild member from user list");
    }

    fn guild_member_update(&self, _: Context, _: Option<Member>, member: Member) {
        user_list::update(member.guild_id, member).expect("failed to update guild member info in user list");
    }

    fn guild_members_chunk(&self, _: Context, guild_id: GuildId, members: HashMap<UserId, Member>) {
        for member in members.values() {
            user_list::add(guild_id, member.clone()).expect("failed to add chunk of guild members to user list");
        }
    }

    fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot { return; } // ignore bots to prevent message loops
        if let Some(err_reply) = match handle_message(ctx, &msg) {
            Ok(()) => None,
            Err(Error::DmOnlyCommand) => Some("to avoid spamming channels, this command is only allowed in direct messages".into()),
            Err(Error::MomirMissingCmc) => Some("missing CMC".into()),
            Err(Error::NoSuchCard(card_name)) => Some(MessageBuilder::default().push("error: no such card: ").push_safe(card_name).build()),
            Err(Error::OwnerCheck) => Some("this command can only be used by the bot owner".into()),
            Err(Error::ParseInt(e)) => Some(MessageBuilder::default().push("invalid number: ").push_safe(e).build()),
            Err(Error::Reqwest(e)) => {
                println!("{}: Message handler returned reqwest error {:?}", Utc::now().format("%Y-%m-%d %H:%M:%S"), e);
                Some("failed to connect to Lore Seeker website, try again later".into()) //TODO check if lore-seeker service is running
            }
            Err(Error::UnknownCommand(cmd)) => Some(MessageBuilder::default().push("unknown command: %").push_safe(cmd).build()),
            Err(e) => {
                println!("{}: Message handler returned error {:?}", Utc::now().format("%Y-%m-%d %H:%M:%S"), e);
                Some("unknown error (cc <@86841168427495424>)".into())
            }
        } {
            msg.reply(&err_reply).expect("failed to send error reply");
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Db(mtg::card::DbError),
    DmOnlyCommand,
    Env(env::VarError),
    Io(io::Error),
    Json(serde_json::Error),
    MissingANode,
    MissingCardDb,
    MissingCardLink,
    MissingCardList,
    MissingContext,
    MissingHref,
    MissingInlineChannels,
    MissingNewline,
    MissingOwners,
    MissingTextNode,
    MomirMissingCmc,
    NoSuchCard(String),
    OwnerCheck,
    ParseInt(std::num::ParseIntError),
    Poison,
    Reqwest(reqwest::Error),
    Serenity(serenity::Error),
    Shlex,
    UnknownCommand(String),
    UrlParse(url::ParseError)
}

impl<'a> From<PoisonError<RwLockReadGuard<'a, serenity::cache::Cache>>> for Error {
    fn from(_: PoisonError<RwLockReadGuard<'a, serenity::cache::Cache>>) -> Error {
        Error::Poison
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Error {
        Error::ParseInt(e)
    }
}

impl From<env::VarError> for Error {
    fn from(e: env::VarError) -> Error {
        Error::Env(e)
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

impl From<mtg::card::DbError> for Error {
    fn from(e: mtg::card::DbError) -> Error {
        Error::Db(e)
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Error {
        Error::Reqwest(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Error {
        Error::Json(e)
    }
}

impl From<serenity::Error> for Error {
    fn from(e: serenity::Error) -> Error {
        Error::Serenity(e)
    }
}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Error {
        Error::UrlParse(e)
    }
}

#[must_use]
fn card_embed(e: CreateEmbed, card: Card, card_url: &Url) -> CreateEmbed {
    e
        .color(match card.rarity().color() {
            (0, 0, 0) => (1, 1, 1), // Discord turns actual black into light gray
            c => c
        })
        .title(if let Some(cost) = card.mana_cost() {
            format!("{} {}", card, with_manamoji(cost))
        } else {
            card.to_string()
        })
        .url(card_url)
        .description({
            let mut description_builder = MessageBuilder::default();
            if let Some(indicator) = card.color_indicator() {
                description_builder = description_builder.emoji(&indicator_emoji(&indicator));
            }
            description_builder
                .push_bold_safe(&card.type_line())
                .push("\n\n")
                .push(with_manamoji(&card.text())) //TODO add support for levelers, fix loyalty costs and quotes, italicize ability words and reminder text
        })
        .fields(
            (if let Some((pow, tou)) = card.pt() {
                Some(("P/T", MessageBuilder::default().push_safe(pow).push("/").push_safe(tou).build(), true))
            } else {
                None
            }).into_iter()
            .chain(card.loyalty().map(|loy| ("Loyalty", loy.to_string(), true)))
            .chain(card.stability().map(|sta| ("Stability", sta.to_string(), true)))
            .chain(card.vanguard_modifiers().map(|(hand, life)| {
                vec![
                    (("Hand modifier", format!("{:+}", hand), true)),
                    (("Life modifier", format!("{:+}", life), true))
                ]
            }).unwrap_or_default())
        )
        .footer(|f| f.text(
            if card.num_printings() >= 100 {
                format!("{} printings", card.num_printings())
            } else {
                card.printings()
                    .into_iter()
                    .map(|printing| format!("{}-{}", printing.set, printing.rarity.short()))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        ))
}

fn eat_user_mention(subj: &mut &str) -> Option<UserId> {
    if !subj.starts_with('<') || !subj.contains('>') {
        return None;
    }
    let mut maybe_mention = String::default();
    let mut chars = subj.chars();
    while let Some(c) = chars.next() {
        maybe_mention.push(c);
        if c == '>' {
            if let Ok(id) = UserId::from_str(&maybe_mention) {
                *subj = &subj[maybe_mention.len()..]; // consume mention text
                return Some(id);
            }
            return None;
        }
    }
    None
}

fn eat_whitespace(subj: &mut &str) {
    while subj.starts_with(' ') {
        *subj = &subj[1..];
    }
}

fn eat_word(subj: &mut &str) -> Option<String> {
    if let Some(word) = next_word(*subj) {
        *subj = &subj[word.len()..];
        eat_whitespace(subj);
        Some(word)
    } else {
        None
    }
}

fn handle_message(ctx: Context, msg: &Message) -> Result<(), Error> {
    let current_user_id = serenity::CACHE.read().user.id;
    let is_inline_channel = {
        // guild in inlineGuilds xor channel in inlineChannels
        let ctx_data = ctx.data.lock();
        let inline_guilds = ctx_data.get::<InlineGuilds>().ok_or(Error::MissingInlineChannels)?;
        msg.guild_id.map_or(false, |guild_id| inline_guilds.contains(&guild_id))
        != ctx_data.get::<InlineChannels>().ok_or(Error::MissingInlineChannels)?.contains(&msg.channel_id)
    };
    let query = &mut if msg.content.starts_with(&current_user_id.mention()) { //TODO allow <@!id> mentions
        let mut query = &msg.content[current_user_id.mention().len()..];
        if query.starts_with(':') { query = &query[1..]; }
        if query.starts_with(' ') { query = &query[1..]; }
        query
    } else if is_inline_channel && msg.content.starts_with('%') || msg.is_private() {
        &msg.content[..]
    } else {
        ""
    };
    if query.starts_with('%') {
        // command
        *query = &query[1..];
        if let Some(cmd_name) = eat_word(query) {
            match &cmd_name[..] {
                "quit" => {
                    owner_check(&ctx, &msg)?;
                    shut_down(&ctx);
                    return Ok(());
                }
                "ping" => {
                    msg.reply("pong")?;
                    return Ok(());
                }
                "check" => {
                    owner_check(&ctx, &msg)?;
                    if !msg.is_private() {
                        return Err(Error::DmOnlyCommand);
                    }
                    msg.reply("reloading database…")?;
                    let mut data = ctx.data.lock();
                    reload_db(&mut data)?;
                    msg.reply("done, resolving query…")?;
                    let (encoded_query, matches) = resolve_query(query)?;
                    msg.reply(&format!("{} cards found: <https://{}/card?q={}>. Checking cards…", matches.len(), HOSTNAME, encoded_query))?;
                    let db = data.get::<CardDb>().ok_or(Error::MissingCardDb)?;
                    let mut oks = 0;
                    let mut errs = 0;
                    for (card_name, card_url) in matches {
                        let card = match db.card(&card_name) {
                            Some(card) => card,
                            None => {
                                msg.reply(&format!("Card not found: {}", card_name))?;
                                errs += 1;
                                continue;
                            }
                        };
                        let check_thread = thread::Builder::new().name("Lore Seeker card check".into()).spawn(move || {
                            let _ = card_embed(CreateEmbed::default(), card, &card_url);
                        })?;
                        match check_thread.join() {
                            Ok(()) => { oks += 1; }
                            Err(_) => {
                                msg.reply(&format!("Error rendering card embed for {}", card_name))?;
                                errs += 1;
                            }
                        }
                    }
                    msg.reply(&format!("{} card embeds successfully rendered, {} errors", oks, errs))?;
                    return Ok(());
                }
                "rand" | "random" => {
                    return handle_query(&ctx, msg, query, true);
                }
                "reload" | "update" => {
                    owner_check(&ctx, &msg)?;
                    let mut data = ctx.data.lock();
                    reload_db(&mut data)?;
                    msg.react("✅")?;
                    return Ok(());
                }
                "booster" | "boosters" | "pack" | "packs" | "sealed" => {
                    while let Some(set_code) = eat_word(query) {
                        let document = {
                            let mut response = reqwest::get(&format!("https://{}/sealed?count[]=1&set[]={}", HOSTNAME, set_code.to_ascii_lowercase()))?.error_for_status()?;
                            let mut response_content = String::default();
                            response.read_to_string(&mut response_content)?;
                            kuchiki::parse_html().one(response_content)
                        };
                        let base_url = Url::parse(&format!("https://{}/", HOSTNAME))?;
                        let set_name = document.select_first(".pack_selection .selection .select2-selection__rendered").ok()
                            .and_then(|node_data| node_data.as_node().as_element()
                                .and_then(|elt| elt.attributes.borrow().get("title").map(ToString::to_string))
                            )
                            .unwrap_or(set_code);
                        let cards = document.select(".card_picture_cell").map_err(|()| Error::MissingCardList)?.map(|cell| {
                            let cell_node: &kuchiki::NodeRef = cell.as_node();
                            let a_node_data = cell_node.select_first("a").map_err(|()| Error::MissingCardLink)?;
                            let a_node = a_node_data.as_node();
                            let a_elt = a_node.as_element().ok_or(Error::MissingANode)?;
                            let href = a_elt.attributes.borrow().get("href").ok_or(Error::MissingHref).and_then(|href| base_url.join(href).map_err(Error::from))?;
                            let img_node_data = a_node.select_first("img").map_err(|()| Error::MissingTextNode)?;
                            let img_node = img_node_data.as_node();
                            let img_elt = img_node.as_element().ok_or(Error::MissingTextNode)?;
                            let card_name = img_elt.attributes.borrow().get("alt").ok_or(Error::MissingTextNode)?.to_string();
                            Ok::<_, Error>((card_name, href))
                        }).collect::<Result<Vec<_>, _>>()?;
                        if let Some(webhook) = ctx.data.lock().get::<WebhookData>()
                            .and_then(|hooks| hooks.get(&msg.channel_id))
                        {
                            webhook.execute(false, |e| e
                                .embeds(vec![
                                    Embed::fake(|e| e
                                        .title(MessageBuilder::default().push_italic_safe(set_name).push(" booster"))
                                        .description({
                                            let mut builder = MessageBuilder::default();
                                            for (card_name, href) in cards {
                                                builder = builder.push('[').push_safe(card_name).push_line(format!("]({})", href));
                                            }
                                            builder
                                        })
                                    )
                                ])
                            )?;
                        } else {
                            msg.channel_id.send_message(|m| {
                                m.embed(|e| e
                                    .title(MessageBuilder::default().push_italic_safe(set_name).push(" booster"))
                                    .description({
                                        let mut builder = MessageBuilder::default();
                                        for (card_name, href) in cards {
                                            builder = builder.push_safe(card_name).push_line(format!(" (<{}>)", href));
                                        }
                                        builder
                                    })
                                )
                            })?;
                        }
                        eat_whitespace(query);
                    }
                    return Ok(());
                }
                "mjt" => {
                    // Mental Judge Tower, for real cards
                    let mut players = Vec::default();
                    while let Some(user_id) = eat_user_mention(query) {
                        players.push(user_id);
                        eat_whitespace(query);
                    }
                    return mental_judge_tower(&msg, players, Some(false), query);
                }
                "cmjt" => {
                    // custom Mental Judge Tower, for custom cards
                    let mut players = Vec::default();
                    while let Some(user_id) = eat_user_mention(query) {
                        players.push(user_id);
                        eat_whitespace(query);
                    }
                    return mental_judge_tower(&msg, players, Some(true), query);
                }
                "fmjt" => {
                    // fusion Mental Judge Tower, for both real and custom cards
                    let mut players = Vec::default();
                    while let Some(user_id) = eat_user_mention(query) {
                        players.push(user_id);
                        eat_whitespace(query);
                    }
                    return mental_judge_tower(&msg, players, None, query);
                }
                "momir" => {
                    // Momir, for real cards
                    let cmc = eat_word(query).ok_or(Error::MomirMissingCmc)?.parse::<usize>()?;
                    eat_whitespace(query);
                    return momir(&ctx, msg, cmc, Some(false), query);
                }
                "cmomir" => {
                    // custom Momir, for custom cards
                    let cmc = eat_word(query).ok_or(Error::MomirMissingCmc)?.parse::<usize>()?;
                    eat_whitespace(query);
                    return momir(&ctx, msg, cmc, Some(true), query);
                }
                "fmomir" => {
                    // fusion Momir, for both real and custom cards
                    let cmc = eat_word(query).ok_or(Error::MomirMissingCmc)?.parse::<usize>()?;
                    eat_whitespace(query);
                    return momir(&ctx, msg, cmc, None, query);
                }
                cmd => {
                    return Err(Error::UnknownCommand(cmd.into()));
                }
            }
        }
    } else if query.len() > 0 {
        handle_query(&ctx, msg, query, false)?;
    } else if is_inline_channel {
        let mut remaining_msg = &msg.content[..];
        while let Some(start_idx) = remaining_msg.find("[[") {
            if let Some(end_offset) = remaining_msg[start_idx..].find("]]") {
                let end_idx = start_idx + end_offset;
                let query = &remaining_msg[start_idx + 2..end_idx];
                if let Some(card) = {
                    let ctx_data = ctx.data.lock();
                    let db = ctx_data.get::<CardDb>().ok_or(Error::MissingCardDb)?;
                    db.card_fuzzy(query)
                } {
                    handle_query_result(&ctx, msg,
                        iter::once((card.to_string(), format!("https://{}/card?q=!{}", HOSTNAME, urlencoding::encode(&card.to_string())).parse().expect("failed to generate card URL"))), //TODO use exact printing URL
                        false,
                        format!("!{}", urlencoding::encode(query)) //TODO fix query
                    )?;
                } else {
                    handle_query(&ctx, msg, query, false)?;
                }
                remaining_msg = &remaining_msg[end_idx + 2..];
            } else {
                break;
            }
        }
    }
    Ok(())
}

fn handle_query(ctx: &Context, msg: &Message, query: &str, random: bool) -> Result<(), Error> {
    let (encoded_query, matches) = resolve_query(query)?;
    handle_query_result(ctx, msg, matches, random, encoded_query)
}

fn handle_query_result(ctx: &Context, msg: &Message, matches: impl IntoIterator<Item = (String, Url)>, random: bool, encoded_query: impl fmt::Display) -> Result<(), Error> {
    let mut matches = matches.into_iter();
    if random {
        let matches_vec = matches.collect::<Vec<_>>();
        if let Some(&(ref card_name, ref card_url)) = matches_vec.choose(&mut thread_rng()) {
            show_single_card(ctx, msg, Some(&format!("{} card{} found, random card", matches_vec.len(), if matches_vec.len() == 1 { "" } else { "s" })), card_name, card_url)?;
        } else {
            msg.reply("no cards found")?;
        }
    } else {
        match (matches.next(), matches.next()) {
            (Some(_), Some(_)) => { msg.reply(&format!("{} cards found: <https://{}/card?q={}>", 2 + matches.count(), HOSTNAME, encoded_query))?; }
            (Some((card_name, card_url)), None) => {
                show_single_card(ctx, msg, None, &card_name, &card_url)?;
            }
            (None, _) => { msg.reply("no cards found")?; }
        }
    }
    Ok(())
}

fn listen_ipc(ctx_arc: Arc<Mutex<Option<Context>>>) -> Result<(), Error> { //TODO change return type to Result<!, Error>
    for stream in TcpListener::bind(IPC_ADDR)?.incoming() {
        let stream = stream?;
        for line in BufReader::new(&stream).lines() {
            let args = shlex::split(&line?).ok_or(Error::Shlex)?;
            match &args[0][..] {
                "quit" => {
                    let ctx_guard = ctx_arc.lock();
                    let ctx = ctx_guard.as_ref().ok_or(Error::MissingContext)?;
                    shut_down(&ctx);
                    thread::sleep(Duration::from_secs(1)); // wait to make sure websockets can be closed cleanly
                    writeln!(&mut &stream, "shutdown complete")?;
                }
                "reload" => {
                    let ctx_guard = ctx_arc.lock();
                    let ctx = ctx_guard.as_ref().ok_or(Error::MissingContext)?;
                    let mut data = ctx.data.lock();
                    reload_db(&mut data)?;
                    writeln!(&mut &stream, "reload complete")?;
                }
                s => { return Err(Error::UnknownCommand(s.to_owned())); }
            }
        }
    }
    unreachable!();
}

fn mental_judge_tower(msg: &Message, mut players: Vec<UserId>, custom: Option<bool>, query: &str) -> Result<(), Error> {
    let mut builder = MessageBuilder::default();
    let mut gen = thread_rng();
    if !players.is_empty() {
        players.shuffle(&mut gen);
        for (i, player) in players.into_iter().enumerate() {
            builder = builder
                .push(if i == 0 { "turn order: " } else { ", " })
                .mention(&player);
        }
        builder = builder.push("\n");
    }
    builder = builder.push(format!(
        "seed: <https://{}/card?q={}+is%3Aprimary+not%3Areprint+sort%3Arand+%28%28-layout%3Asplit+-layout%3Aaftermath%29+or+number%3A%2Fa%2F%29",
        HOSTNAME,
        match custom {
            Some(true) => "st%3Acustom",
            Some(false) => "%28f%3AVintage+or+%28banned%3AVintage+-o%3A%2F%5CWante%5CW%2F%29%29",
            None => "%28f%3AVintage+or+%28banned%3AVintage+-o%3A%2F%5CWante%5CW%2F%29+or+st%3Acustom%29"
        }
    ));
    if !query.is_empty() {
        builder = builder.push("+").push(urlencoding::encode(query));
    }
    builder = builder.push(format!(
        "&random_seed={:08x}{:08x}>",
        gen.gen_range(0, 0x1_0000_0000_u64),
        gen.gen_range(0, 0x1_0000_0000_u64)
    ));
    msg.reply(&builder.build())?;
    Ok(())
}

fn momir(ctx: &Context, msg: &Message, cmc: usize, custom: Option<bool>, query: &str) -> Result<(), Error> {
    let query = format!(
        "is:primary t:creature cmc={} {}{}{}",
        cmc,
        match custom {
            Some(true) => "is:custom",
            Some(false) => "f:Vintage",
            None => "(is:custom or f:Vintage)"
        },
        if query.is_empty() { "" } else { " " },
        query
    );
    handle_query(ctx, msg, &query, true)
}

fn next_word(subj: &str) -> Option<String> {
    let mut word = String::default();
    for c in subj.chars() {
        if c == ' ' { break; }
        word.push(c);
    }
    if word.is_empty() { None } else { Some(word) }
}

fn notify_ipc_crash(e: Error) {
    let mut child = Command::new("ssmtp")
        .arg("fenhl@wurstmineberg.de")
        .stdin(Stdio::piped())
        .spawn()
        .expect("failed to spawn ssmtp");
    {
        let stdin = child.stdin.as_mut().expect("failed to open ssmtp stdin");
        write!(
            stdin,
            "To: fenhl@fenhl.net\nFrom: {}@{}\nSubject: Lore Seeker IPC thread crashed\n\nLore Seeker IPC thread crashed with the following error:\n{:?}\n",
            whoami::username(),
            whoami::hostname(),
            e
        ).expect("failed to write to ssmtp stdin");
    }
    child.wait().expect("failed to wait for ssmtp subprocess"); //TODO check exit status
}

fn owner_check(ctx: &Context, msg: &Message) -> Result<(), Error> {
    let data = ctx.data.lock();
    let owners = data.get::<Owners>().ok_or(Error::MissingOwners)?;
    if owners.contains(&msg.author.id) {
        Ok(())
    } else {
        Err(Error::OwnerCheck)
    }
}

fn reload_db(ctx_data: &mut ShareMap) -> Result<(), Error> {
    let db = Db::from_sets_dir("/opt/git/github.com/fenhl/lore-seeker/stage/data/sets")?;
    ctx_data.insert::<CardDb>(db);
    Ok(())
}

fn resolve_query(query: &str) -> Result<(String, Vec<(String, Url)>), Error> {
    let encoded_query = urlencoding::encode(if query.is_empty() { "*" } else { query });
    let document = {
        let mut response = reqwest::get(&format!("http://localhost:18803/list?q={}", encoded_query))?.error_for_status()?;
        let mut response_content = String::default();
        response.read_to_string(&mut response_content)?;
        kuchiki::parse_html().one(response_content)
    };
    let card_list_data = document.select_first("ul#search-result").map_err(|()| Error::MissingCardList)?;
    let card_list = card_list_data.as_node();
    let mut matches = Vec::default();
    let base_url = Url::parse(&format!("https://{}/", HOSTNAME))?;
    for li_node_data in card_list.select("li").map_err(|()| Error::MissingCardList)? {
        let li_node = li_node_data.as_node();
        let a_node_data = li_node.select_first("a").map_err(|()| Error::MissingCardLink)?;
        let a_node = a_node_data.as_node();
        let a_elt = a_node.as_element().ok_or(Error::MissingANode)?;
        let text_node = a_node.first_child().ok_or(Error::MissingTextNode)?;
        let href = a_elt.attributes.borrow().get("href").ok_or(Error::MissingHref).and_then(|href| base_url.join(href).map_err(Error::from))?;
        let text = text_node.as_text().ok_or(Error::MissingTextNode)?;
        matches.push((text.borrow().trim().to_owned(), href));
    }
    Ok((encoded_query, matches))
}

fn send_ipc_command<T: fmt::Display, I: IntoIterator<Item = T>>(cmd: I) -> Result<String, Error> {
    let mut stream = TcpStream::connect(IPC_ADDR)?;
    writeln!(&mut stream, "{}", cmd.into_iter().map(|arg| shlex::quote(&arg.to_string()).into_owned()).collect::<Vec<_>>().join(" "))?;
    let mut buf = String::default();
    BufReader::new(stream).read_line(&mut buf)?;
    if buf.pop() != Some('\n') { return Err(Error::MissingNewline) }
    Ok(buf)
}

fn show_single_card(ctx: &Context, msg: &Message, reply_text: Option<&str>, card_name: &str, card_url: &Url) -> Result<(), Error> {
    let card = {
        let data = ctx.data.lock();
        data.get::<CardDb>().and_then(|db| db.card(card_name))
    };
    msg.channel_id.send_message(|m| {
        let m = match (reply_text, card.is_some()) {
            (None, true) => m,
            (_, _) => {
                let reply_text = reply_text.unwrap_or("1 card found");
                m.content(MessageBuilder::default().mention(&msg.author).push(format!(": {}: <{}>", reply_text, card_url)))
            }
        };
        if let Some(card) = card {
            m.embed(|e| card_embed(e, card, card_url))
        } else {
            m
        }
    })?;
    Ok(())
}

/// Utility function to shut down all shards.
pub fn shut_down(ctx: &Context) {
    ctx.invisible(); // hack to prevent the bot showing as online when it's not
    let data = ctx.data.lock();
    let mut shard_manager = data.get::<ShardManagerContainer>().expect("missing shard manager").lock();
    shard_manager.shutdown_all();
}

fn main() -> Result<(), Error> {
    let mut args = env::args().peekable();
    let _ = args.next(); // ignore executable name
    if args.peek().is_some() {
        println!("{}", send_ipc_command(args)?);
    } else {
        // read config
        let config = serde_json::from_reader::<_, Config>(File::open("/usr/local/share/fenhl/lore-seeker/config.json")?)?;
        let handler = Handler::default();
        let ctx_arc = handler.0.clone();
        let mut client = Client::new(&config.bot_token, handler)?;
        let owners = {
            let mut owners = HashSet::default();
            owners.insert(serenity::http::get_current_application_info()?.owner.id);
            owners
        };
        {
            let mut data = client.data.lock();
            data.insert::<Owners>(owners);
            data.insert::<InlineChannels>(config.inline_channels);
            data.insert::<InlineGuilds>(config.inline_guilds);
            data.insert::<ShardManagerContainer>(Arc::clone(&client.shard_manager));
            data.insert::<WebhookData>(config.webhooks.into_iter().map(|(chan_id, webhook_data)| webhook_data.to_webhook().map(|webhook| (chan_id, webhook))).collect::<serenity::Result<_>>()?);
            // load cards before going online
            print!("[....] loading cards");
            io::stdout().flush()?;
            let db = Db::from_sets_dir("/opt/git/github.com/fenhl/lore-seeker/stage/data/sets")?;
            assert!(db.card("Dryad Arbor").ok_or(Error::NoSuchCard("Dryad Arbor".to_owned()))?.loyalty().is_none());
            let num_cards = db.into_iter().count();
            data.insert::<CardDb>(db);
            println!("\r[ ok ] {} cards loaded", num_cards);
        }
        // listen for IPC commands
        {
            thread::Builder::new().name("Lore Seeker IPC".into()).spawn(move || {
                if let Err(e) = listen_ipc(ctx_arc) { //TODO remove `if` after changing from `()` to `!`
                    eprintln!("{:?}", e);
                    notify_ipc_crash(e);
                }
            })?;
        }
        // connect to Discord
        client.start_autosharded()?;
        thread::sleep(Duration::from_secs(1)); // wait to make sure websockets can be closed cleanly
    }
    Ok(())
}
