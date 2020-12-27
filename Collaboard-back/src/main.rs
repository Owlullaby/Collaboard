#[macro_use]
extern crate dotenv_codegen;

use std::time::{Duration, Instant};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
//use std::{cell::{RefCell, Cell}, rc::Rc};

use actix::*;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder,};
use actix_web_actors::ws;
use actix_files as fs;

use futures::future;

use rand::{self, rngs::ThreadRng, Rng};
use rand::distributions::Alphanumeric;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use json_patch::merge;

use tera::{Tera, Context as ctx};

/// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
/// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);
const WS: &str = dotenv!("WS");
const IP: &str = dotenv!("IP");

// Hash - the server actor //
pub struct Hash {
	sessions: HashMap<usize, Recipient<DrawData>>,
	rooms: HashMap<String, HashSet<usize>>,
	rng: ThreadRng,
	buser: Arc<Mutex<Vec<String>>>,
	broom: Arc<Mutex<Vec<String>>>,
}

impl Hash{
	fn send_message(&self, room: &str, message: &str, skip_id: usize) {
		if let Some(sessions) = self.rooms.get(room) {
			for id in sessions {
 				if *id != skip_id {
					if let Some(addr) = self.sessions.get(id) {
						let _ = addr.do_send(DrawData{id: skip_id, data: message.to_owned()});
						println!("data sent");
					}
				}
			}
		}
	}
}

impl Actor for Hash{
	type Context = Context<Self>;
}

impl Default for Hash {
    fn default() -> Hash {
		let mut rooms = HashMap::new();
		rooms.insert("Main".to_owned(), HashSet::new());
        Hash {
			sessions: HashMap::new(),
			rooms,
			rng: rand::thread_rng(),
			buser: Arc::new(Mutex::new(vec![])),
			broom: Arc::new(Mutex::new(vec![])),
        }
    }
}

// All the Handler for Hash //
#[derive(Message)]
#[rtype(result = "()")]
struct Bridge {
	username: String,
	roomcode: String,
}

impl Handler<Bridge> for Hash {
    type Result = ();

    fn handle(&mut self, msg: Bridge, _: &mut Context<Self>) {
		//self.address = msg.addr;
		self.buser.lock().unwrap().push(msg.username);
		self.broom.lock().unwrap().push(msg.roomcode);
    }
}

#[derive(MessageResponse)]
pub struct Response{
	pub id: usize,
	pub room: String,
	pub username: String,
}

#[derive(Message)]
#[rtype(result = "Response")]
pub struct Join {
    pub username: String,
	pub room: String,
	pub addr: Recipient<DrawData>,
	
}

impl Handler<Join> for Hash {
    type Result = Response;

    fn handle(&mut self, msg: Join, _: &mut Context<Self>) -> Self::Result {
		let id = self.rng.gen::<usize>();
		println!("{}", id);
		//let username = &msg.username;
		let username = if self.buser.lock().unwrap().is_empty(){
			msg.username
			}else{
			self.buser.lock().unwrap().pop().unwrap()
			// println!("{}", username);
		};
		println!("{}", username);
        self.sessions.insert(id, msg.addr);

		// let room = &msg.room;
		let room = if self.broom.lock().unwrap().is_empty(){
			msg.room
			}else{
			self.broom.lock().unwrap().pop().unwrap()
			// println!("{}", username);
		};
		// self.rooms.insert(room.clone(), HashSet::new());
		println!("{}", room);
        self.rooms
            .entry(room.clone())
            .or_insert(HashSet::new())
            .insert(id);

		Response{id: id, room: room.to_owned(), username: username.to_owned()}
	}

}

#[derive(Message)]
#[rtype(result = "()")]
pub struct HandleMessage {
	pub room: String,
	pub msg: String,
	pub id: usize,
}

impl Handler<HandleMessage> for Hash{
    type Result = ();
    fn handle(&mut self, msg: HandleMessage, _: &mut Context<Self>) {
		self.send_message(&msg.room, &msg.msg, msg.id);
	}
}


// Draw - the drawing actor //
struct Draw{
	hb: Instant,
	addr: Addr<Hash>,
	id: usize,
	username: String,
	room: String,
	//pressed: bool,
	hashhash: HashMap<usize,Vec<(f64,f64)>>,
}

impl Draw {
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b""); //byte string
        });
	}

}

impl Actor for Draw{
	type Context = ws::WebsocketContext<Self>;
	fn started(&mut self, ctx: &mut Self::Context) {
        // we'll start heartbeat process on session start.
		self.hb(ctx);
		let addr = ctx.address(); //wschatsession Addr

        self.addr //chatserver Addr
            .send(Join {
				addr: addr.recipient(),
				username: self.username.clone(),
				room: self.room.clone(),
            })
            .into_actor(self)
            .then(|res, act, ctx| {
                match res {
					Ok(res) => {
						act.id = res.id;
						act.room = res.room;
						act.username = res.username;
						println!("drawing");
					},
                    // something is wrong with chat server
                    _ => ctx.stop(),
                }
                fut::ready(())
            })
			.wait(ctx);
	}
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Draw {
	fn handle(
		&mut self,
		msg: Result<ws::Message, ws::ProtocolError>,
		ctx: &mut Self::Context,
	) {
		match msg {
			Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
			Ok(ws::Message::Text(text)) =>{
				self.addr.do_send(HandleMessage{
					id: self.id,
					room: self.room.clone(),
					msg: text,
				});
			} 
			Ok(ws::Message::Binary(bin)) => {
				//ctx.binary(bin);
				println!("{:?}", &bin);
			},
			_ => ctx.stop(),	
		}
	}
}

// All the Handler for Draw //
#[derive(Message)]
#[rtype(result = "()")]
// pub struct DrawData(pub String);
pub struct DrawData{
	id: usize,
	data: String,
}
#[derive(Serialize,Deserialize)]
struct ProcessedData{
	id: usize,
	points: (f64, f64),
	events: String,
	mode_set: String,
}

impl Handler<DrawData> for Draw{
	type Result = ();

    fn handle(&mut self, msg: DrawData, ctx: &mut Self::Context) {
		// error as missing id field
		// let mut data: ReceivedData = serde_json::from_str(&msg.data).unwrap();
		let mut data: Value = serde_json::from_str(&msg.data).unwrap();
		println!("{}", serde_json::to_string_pretty(&data).expect("unable to serialize"));
		// let data2: Value = serde_json::to_value(&msg.id).unwrap();
		let data2 : Value = json!({"id": &msg.id});
		println!("{}", serde_json::to_string_pretty(&data2).expect("unable to serialize"));
		merge(&mut data, &data2);
		// add in id for drawdata
		// *data.pointer_mut("/id").unwrap() = msg.id.into();
		// cannot use let as let is used to dep=find a variable
		let processed: ProcessedData = serde_json::from_value(data).unwrap();
		let id = processed.id;
		let points = processed.points;
		let events = processed.events;
		let mode_set = processed.mode_set;

		println!("processed, putting into hashmap");
		//let mut hashhash: HashHash = HashHash{hashhash: HashMap::new()};
		//let mut hashhash = HashMap::new();
		if events == "mousedown"{
			if self.hashhash.contains_key(&id) == false{
				self.hashhash.insert(id, Vec::new());
			}
		self.hashhash
			.entry(id)
			.or_insert(Vec::new())
			.push(points);
		println!("{:?}", self.hashhash.get(&id));
		println!("{}", self.hashhash.get(&id).unwrap().len());
		println!("mousedown pushed");
		} else if events == "mousemove"{
			self.hashhash
				.entry(id)
				.or_insert(Vec::new())
				.push(points);
				println!("mousemove pushed");
				println!("{}", self.hashhash.get(&id).unwrap().len());
				println!("{:?}", self.hashhash.get(&id));	
		} else if events == "mouseup"{
			self.hashhash
				.entry(id)
				.or_insert(Vec::new())
				.push(points);
			println!("{:?}", self.hashhash.get(&id));
			println!("{}", self.hashhash.get(&id).unwrap().len());
			println!("mouseup pushed");
			if let Some(stroke) = self.hashhash.remove(&id){
				let d = json!({
					"mode_set": mode_set,
					"stroke": stroke,
				});
				println!("{}", serde_json::to_string_pretty(&d).expect("unable to serialize"));
				ctx.text(serde_json::to_string(&d).unwrap());
				//hashhash.remove(&id);
			}
		}
		println!("msg sent to collaboard");
    }
}

#[derive(Deserialize)]
pub struct UserData {
	usernamecreate: String,
}

#[derive(Deserialize)]
pub struct JoinData {
	usernamejoin: String,
	roomcode: String,
}

async fn create_room(params: web::Form<UserData>, srv: web::Data<Addr<Hash>>, data: web::Data<ShowRoom>) -> impl Responder{
	let addr = srv.get_ref().to_owned();
	println!("room created");
	let room: String = rand::thread_rng().sample_iter(Alphanumeric).take(5).collect();
	addr.do_send(Bridge{username: params.usernamecreate.to_owned(), roomcode: room.to_owned()});
	let mut ctx = ctx::new(); //Tera::Context as ctx
	ctx.insert("roomcode", &room);
	let rendered = data.tmpl.render("draw.html", &ctx).unwrap();
	HttpResponse::Ok().body(rendered)
}

async fn join_room(params: web::Form<JoinData>, srv: web::Data<Addr<Hash>>, data: web::Data<ShowRoom>) -> impl Responder{
	let addr = srv.get_ref().to_owned();
	addr.do_send(Bridge{username: params.usernamejoin.to_owned(), roomcode: params.roomcode.to_owned()});
	println!("joined room");
	let mut ctx = ctx::new();
	ctx.insert("roomcode", &params.roomcode);
	let rendered = data.tmpl.render("draw.html", &ctx).unwrap();
	HttpResponse::Ok().body(rendered)
}

async fn drawing(req: HttpRequest, stream: web::Payload, srv: web::Data<Addr<Hash>>) -> Result<HttpResponse, Error>{
	println!("{:?}", req);
	let (_addr, resp) = ws::start_with_addr(
		Draw{ 
			hb: Instant::now(), 
			addr: srv.get_ref().clone(), 
			id:0, 
			username: "Annonymous".to_owned(), 
			room: "Main".to_owned(),
			//pressed: pressed.get(),
			hashhash: HashMap::new(),
		}, 
			&req, stream)?;
	Ok(resp)
}

async fn index() -> Result<fs::NamedFile, Error>{
	let path: PathBuf = PathBuf::from("./static/index.html");
	Ok(fs::NamedFile::open(path)?)
}

struct ShowRoom{
	tmpl: Tera
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
	println!("Collaboard started!");
	macro_rules! enclose {
		( ($( $x:ident ),*) $y:expr ) => {
			{
				$(let $x = $x.clone();)*
				$y
			}
		};
	}

	let tera = match Tera::new("template/**/*.html") {
		Ok(t) => t,
		Err(e) => {
			println!("Parsing error(s): {}", e);
			::std::process::exit(1);
		}
	};

	let server = Hash::default().start();
	let s1 = HttpServer::new(enclose!((server, tera)move || {
		App::new()
			.data(ShowRoom{tmpl: tera.clone()})
			.data(server.clone())
			.service(web::resource("/ws/").to(drawing))
	}))
	//.bind("127.0.0.1:8080")?
	.bind(WS)?
	.run();

	let s2 = HttpServer::new(enclose!((server, tera)move || {
			App::new()
			.data(ShowRoom{tmpl: tera.clone()})
			.data(server.clone())
			.service(web::resource("/").route(web::get().to(index)))
			.service(web::resource("/create").route(web::post().to(create_room)))
			.service(web::resource("/join").route(web::post().to(join_room)))
			.service(fs::Files::new("/", "./static"))
    }))
		//.bind("127.0.0.1:5000")?
		.bind(IP)?
		.run();
		future::try_join(s1,s2).await?;

Ok(())
}