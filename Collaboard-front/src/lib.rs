extern crate stdweb;
#[macro_use]
extern crate dotenv_codegen;

use std::{cell::Cell, rc::Rc};
use wasm_bindgen::prelude::*;
use stdweb::{js, console};
use stdweb::traits::*;
use stdweb::unstable::TryInto;
use stdweb::web::{document, WebSocket, CanvasRenderingContext2d};
use stdweb::web::html_element::{CanvasElement};
use stdweb::web::event::{TouchStart, TouchMove, TouchEnd, MouseDownEvent, MouseMoveEvent, MouseUpEvent, SocketMessageEvent, IMessageEvent, ClickEvent};

//additional requirements for eraser
use std::f64::consts::PI;
use stdweb::web::CompositeOperation::{DestinationOut, SourceOver};
use stdweb::web::FillRule::NonZero;
use stdweb::web::HtmlElement;

use serde::{Serialize, Deserialize};
use dotenv;

macro_rules! enclose {
    ( ($( $x:ident ),*) $y:expr ) => {
        {
            $(let $x = $x.clone();)*
            $y
        }
    };
}

#[derive(Serialize, Deserialize)]
struct Data<'a> {
 	//x: f64,
	//y: f64,
	points: (f64, f64),
	events: &'a str,
	mode_set: &'a str,
}

#[derive(Serialize, Deserialize)]
struct DataReceived<'a> {
	//events: &'a str,
	//id: u64,
	mode_set: &'a str,
 	//x: f64,
	//y: f64,
	stroke: Vec<(f64, f64)>,
}

#[wasm_bindgen(start)]
pub fn main() {
	//get value from WS key in dotenv file
	let addr = dotenv!("WS");
	let ws: WebSocket = WebSocket::new(addr).unwrap();
	let canvas: CanvasElement = document().query_selector( "#canvas" ).unwrap().unwrap().try_into().unwrap();
	let context: CanvasRenderingContext2d = canvas.get_context().unwrap();
	let mode = Rc::new(Cell::new("pencil"));
	let pencil: HtmlElement = document().get_element_by_id("pencil").unwrap().try_into().unwrap();
	let eraser: HtmlElement = document().get_element_by_id("eraser").unwrap().try_into().unwrap();
	let current = Rc::new(Cell::new((0.0,0.0)));


	pencil.add_event_listener(enclose!((mode)move |_: ClickEvent|{
		let status: HtmlElement = document().get_element_by_id("status").unwrap().try_into().unwrap();
		mode.set("pencil");
		js!{
			@{status}.innerHTML="Pencil selected.";
			console.log("pencil mode!");
		}
	}));

	eraser.add_event_listener(enclose!((mode)move |_: ClickEvent|{
		let status: HtmlElement = document().get_element_by_id("status").unwrap().try_into().unwrap();
		mode.set("eraser");
		js!{
			@{status}.innerHTML="Eraser selected.";
			console.log("eraser mode!");
		}
	}));
	
	//draw events 1: mousedown
	let pressed = Rc::new(Cell::new(false));
	canvas.add_event_listener(enclose!((ws, mode, pressed, current)move |event: MouseDownEvent| {
		// let mx = canvas.get_bounding_client_rect().get_left();
		// let my = canvas.get_bounding_client_rect().get_top();
		// let x = (event.offset_x() as f64 ) + mx;
		// let y = (event.offset_y() as f64 ) + my;
		let x = event.offset_x();
		let y = event.offset_y();
		let events: &str = "mousedown";
		let data = Data {
			points: (x,y),
			events: events,
			mode_set: mode.get()
		};
		let se_data = serde_json::to_string(&data).unwrap();
		if mode.get() == "pencil" {
			current.set((x,y));
		} else {
			current.set((x,y));
		}
		
		pressed.set(true);
		ws.send_text(&se_data).unwrap();
		//js!(console.log("collaboard: to test is functional"));
	}));

	//draw events 2: mousemove
    canvas.add_event_listener(enclose!((mode,ws, pressed, current, context)move |event: MouseMoveEvent| {
		if pressed.get(){
			// let mx = canvas.get_bounding_client_rect().get_left();
			// let my = canvas.get_bounding_client_rect().get_top();
			// let x = (event.offset_x() as f64 ) + mx;
			// let y = (event.offset_y() as f64 ) + my;
			let x = event.offset_x();
			let y = event.offset_y();
			let events: &str = "mousemove";
			let data = Data {
				points:(x,y),
				events: events,
				mode_set: mode.get()
			};
			let se_data = serde_json::to_string(&data).unwrap();
			if mode.get() == "pencil" {
				let (a,b) = current.get();
				let context =  context.clone();
				context.begin_path();
				context.set_global_composite_operation(SourceOver);
				context.move_to(a,b);
				context.line_to(x, y);
				context.stroke();
				//draw(data.points, data.events);
				current.set((x,y));
			} else {
				//erase(data.points, data.events);
				let (a,b) = current.get();
				let context = context.clone();
				context.begin_path();
				context.set_global_composite_operation(DestinationOut);
	 			context.arc(x, y, 30.0, 0.0*PI, 2.0*PI, true);
	 			context.fill(NonZero);
	 			context.move_to(a, b);
	 			context.line_to(x, y);
				context.stroke();
				current.set((x,y));
			}
			//send coordinates and events here
			ws.send_text(&se_data).unwrap();
		}
	}));

	//draw event 3: stop
    canvas.add_event_listener(enclose!((ws, mode, pressed, current, context)move |event: MouseUpEvent| {
		// let mx = canvas.get_bounding_client_rect().get_left();
		// let my = canvas.get_bounding_client_rect().get_top();
		// let x = (event.offset_x() as f64 ) + mx;
		// let y = (event.offset_y() as f64 ) + my;
		let x = event.offset_x();
		let y = event.offset_y();
		let events: &str = "mouseup";
		let data = Data {
			//x: x,
			//y: y,
			points: (x,y),
			events: events,
			mode_set: mode.get()
		};
		let se_data = serde_json::to_string(&data).unwrap();
		if mode.get() == "pencil" {
			let (c,d) = current.get();
			context.begin_path();
			context.set_global_composite_operation(SourceOver);
			context.move_to(c,d);
			context.line_to(x,y);
			context.stroke();
			//draw(data.points, data.events);
		} else {
			//erase(data.points, data.events);
			let (c,d) = current.get();
			context.begin_path();
			context.set_global_composite_operation(DestinationOut);
	 		context.arc(x, y, 30.0, 0.0*PI, 2.0*PI, true);
	 		context.fill(NonZero);
			context.move_to(c,d);
			context.line_to(x,y);
			context.stroke();
		}
		ws.send_text(&se_data).unwrap();
		//console!(log, "stroke data sent to serving");
		pressed.set(false);
	}));

	// added in support for touchscreeen devices //
	
	// touch start
	canvas.add_event_listener(enclose!((ws, mode, pressed, current)move |event: TouchStart| {
		event.prevent_default();
		let event = event.changed_touches();
		let x = event[0].page_x();
		let y = event[0].page_y();
		let events: &str = "mousedown";
		let data = Data {
			//x: x,
			//y: y,
			points: (x,y),
			events: events,
			mode_set: mode.get()
		};
		let se_data = serde_json::to_string(&data).unwrap();
		if mode.get() == "pencil" {
			current.set((x,y));
			//draw(data.points, data.events);
		} else {
			//erase(data.points, data.events);
			current.set((x,y));
		}
		
		pressed.set(true);
		//send coordinates and events here
		ws.send_text(&se_data).unwrap();
	
	}));

	// touch move //
	canvas.add_event_listener(enclose!((ws, mode, current, pressed, context)move |event: TouchMove| {
		event.prevent_default();
		if pressed.get(){
			let event = event.changed_touches();
			let x = event[0].page_x();
			let y = event[0].page_y();
			let events: &str = "mousemove";
			let data = Data {
				points: (x,y),
				events: events,
				mode_set: mode.get()
			};
			let se_data = serde_json::to_string(&data).unwrap();
			if mode.get() == "pencil" {
				let (a,b) = current.get();
				let context =  context.clone();
				context.begin_path();
				context.set_global_composite_operation(SourceOver);
				context.move_to(a,b);
				context.line_to(x, y);
				context.stroke();
				//draw(data.points, data.events);
				current.set((x,y));
			} else {
				//erase(data.points, data.events);
				let (a,b) = current.get();
				context.begin_path();
				context.set_global_composite_operation(DestinationOut);
				context.arc(x, y, 30.0, 0.0*PI, 2.0*PI, true);
				context.fill(NonZero);
				context.move_to(a,b);
				context.line_to(x,y);
				context.stroke();
				current.set((x,y));
			}
			//send coordinates and events here
			ws.send_text(&se_data).unwrap();
		}
	}));

		// touch end //
		canvas.add_event_listener(enclose!((ws, mode, current, pressed, context)move |event: TouchEnd| {
			event.prevent_default();
			let event = event.changed_touches();
			let x = event[0].page_x();
			let y = event[0].page_y();
			const EVENTS: &str = "mouseup";
			let data = Data {
				points: (x,y),
				events: EVENTS,
				mode_set: mode.get()
			};
			let se_data = serde_json::to_string(&data).unwrap();
			if mode.get() == "pencil" {
				let (c,d) = current.get();
				context.begin_path();
				context.set_global_composite_operation(SourceOver);
				context.move_to(c,d);
				context.line_to(x,y);
				context.stroke();
				//draw(data.points, data.events);
			} else {
				// erase(data.points, data.events);
				let (c,d) = current.get();
				context.begin_path();
				context.set_global_composite_operation(DestinationOut);
				context.arc(x, y, 30.0, 0.0*PI, 2.0*PI, true);
				context.fill(NonZero);
				context.move_to(c,d);
				context.line_to(x,y);
				context.stroke();
			}
			pressed.set(false);
			//send coordinates and events here
			//ws.send_text(&se_data).unwrap();
			// let ws = socket();
			// let ws = &ws;
			ws.send_text(&se_data).unwrap();
		}));

	// listening for events in the socket //
	ws.add_event_listener(move |event: SocketMessageEvent| {
		//let t = std::thread::spawn(move || {
		let receive = event.data().into_text().unwrap();
		//console!(log, &receive);
		//js!(console.log("collaboard: receiving data"));
		
		let received_data: DataReceived = serde_json::from_str(&receive).unwrap();
		//js!(console.log("collaboard receiving socket msg"));
		let stroke = received_data.stroke;
		let mode_set = received_data.mode_set;
		if mode_set == "pencil" {
			trace(stroke);
			console!(log, "tracing");
		} else if mode_set == "eraser"{
			remove(stroke);
		}
	});
}

// tracing stroke paths on canvas //
pub fn trace(stroke:Vec<(f64, f64)>){
		let canvas: CanvasElement = document().query_selector("#canvas").unwrap().unwrap().try_into().unwrap();
		let context: CanvasRenderingContext2d = canvas.get_context().unwrap();
		let context = Rc::new(context);
		//let ws = WebSocket::new("ws://127.0.0.1:8080").unwrap();
		//let pressed = Rc::new(Cell::new(false));
		context.begin_path();
		context.move_to(stroke[0].0, stroke[0].1);
		for (x, y) in stroke {
			//pressed.set(true);
			context.set_global_composite_operation(SourceOver);
			context.line_to(x,y);
			context.stroke();
			context.begin_path();
			context.move_to(x,y);
			// context.line_to(x, y);
			// context.stroke();
			//pressed.set(false);
		}
		context.stroke();
		//console!(log, "tracing done");
	
}

// removing stroke path from canvas //
pub fn remove(stroke:Vec<(f64, f64)>){
	let canvas: CanvasElement = document().query_selector("#canvas").unwrap().unwrap().try_into().unwrap();
	let context: CanvasRenderingContext2d = canvas.get_context().unwrap();
	//let context = Rc::new(context);
	context.begin_path();
	context.move_to(stroke[0].0, stroke[0].1);
	for (x, y) in stroke {
		context.set_global_composite_operation(DestinationOut);
		context.arc(x, y, 30.0, 0.0*PI, 2.0*PI, true);
		context.fill(NonZero);
		context.line_to(x,y);
	}
	context.stroke();
}

// fn set_panic_hook(){
// 	std::panic::set_hook(Box::new(|info|{
// 		stdweb::print_error_panic(&info.to_string());
// 	}));
// }