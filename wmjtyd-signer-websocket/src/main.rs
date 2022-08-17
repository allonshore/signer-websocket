use axum::{
    body::StreamBody,
    extract::{
        ws::{Message as RawMessage, WebSocket, WebSocketUpgrade},
        Extension, TypedHeader,
    },
    http::{header, StatusCode},
    response::{Response, IntoResponse},
    routing::{get, post},
    Json, Router
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{net::SocketAddr};
use tower_http::{
    services::ServeDir,
    trace::{DefaultMakeSpan, TraceLayer},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::{
    collections::HashMap,
    sync::{Arc},
};
use std::{ops::Add, sync::Mutex};
use axum::http::HeaderValue;
use tokio_util::io::ReaderStream;

use wmjtyd_signer_websocket::{config::config::ApplicationConfig, init_config};
use wmjtyd_libstock::message::{
    traits::{Connect, StreamExt, Subscribe},
    zeromq::ZeromqSubscriber,
};
use async_channel::{bounded as channel, TryRecvError};
use async_channel::{Receiver, Sender};
// use crossbeam_channel::{Receiver, Sender};

// use crossbeam_channel::unbounded as channel;
use rand::prelude::*;
// #[derive(Default)]
// // pub struct AppState {
// //     pub receiver: HashMap<i64, WebSocket>,
// // }
type SenderAction = Sender<SocketAction>;
type ReceiverAction = Receiver<SocketAction>;

type TxJSON = Sender<String>;
type RxJSON = Receiver<String>;

pub enum SocketAction {
    Add(i64,TxJSON),
    Remove(i64)
}

pub async fn updateA(receiverAction:ReceiverAction){
    let mut senderMap:HashMap<i64,TxJSON> = HashMap::new();
    let sub = ZeromqSubscriber::new();
    let ipc = "ipc:///tmp/signer_all.ipc";
    if sub.is_err() {
        tracing::error!("sub init");
        return;
    }
    let mut sub = sub.unwrap();
    if sub.connect(ipc).is_err() || sub.subscribe(b"").is_err() {
        tracing::error!("sub connect");
        return;
    }

    loop {
        // let cc = rx.recv();
        tokio::select! {
            val = receiverAction.recv() =>{
                // if val.is_error() {
                //     break;
                // }
                let val = val.unwrap();
                 match val{
                    SocketAction::Add(id,tx_json)=> {
                        senderMap.insert(id,tx_json);
                    },
                    SocketAction::Remove(id)=> {
                        senderMap.remove(&id);
                    }
                 }
                //  let temp =   val.lock();
                //  webSocketMap.insert(temp.0,temp.1)
            }
            message = StreamExt::next(&mut sub) =>{
                if message.is_none() {
                    break;
                }
        
                tracing::debug!("get data ipc: {}", ipc);
        
                match message.unwrap() {
                    Ok(message) => {
                        let temp = String::from_utf8(message).unwrap();
         
                        for (_,tx) in senderMap.iter() {
                            tx.send(temp.to_owned()).await.unwrap();

                        }
                        // fot val in receiverList
                        // socket.send(message);
                        // if tx.send((ipc.to_owned(), message)).is_err() {
                        //     tracing::error!("send data");
                        // }
                    }
                    Err(_) => {
                        tracing::error!("message zero");
                        break;
                    }
                }
            }
        }
        // 数据 payload
        // let message = StreamExt::next(&mut sub).await;
       
    }
    tracing::debug!("loop end")    
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let application_config = init_config().await;
    println!("Hello, world!");
    // 创建一个消息通道, 返回一个元组：(发送者，接收者)
    let (SenderAction, ReceiverAction) =channel(10);
     // 创建线程，并发送消息
     tokio::task::spawn(updateA(ReceiverAction) );
     let port = application_config.port;
    let app = Router::new()
    .route("/", get(|| async { "Hello, World!" }))
    // .route("/file", post(handler))
    //绑定websocket路由
    .route("/ws", get(ws_handler))
    .layer(
        TraceLayer::new_for_http()
            .make_span_with(DefaultMakeSpan::default().include_headers(true)),
    )
    .layer(Extension(SenderAction))
    .layer(Extension(Arc::new(ReceiverList::default())))
    .layer(Extension(application_config));
    // updateA();
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        tracing::debug!("listening on {}", addr);
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
}
async fn ws_handler(
    ws: WebSocketUpgrade,
    user_agent: Option<TypedHeader<headers::UserAgent>>,
    Extension(senderAction): Extension<Sender<SocketAction>>,
    // Extension(receiverList): Extension<Arc<ReceiverList>>,
    Extension(application_config): Extension<ApplicationConfig>,
) -> impl IntoResponse {
    if let Some(TypedHeader(user_agent)) = user_agent {
        println!("`{}` connected", user_agent.as_str());
    }
    let senderAction = senderAction.clone();
   


    // let sen = Mutex::new(senderAction); 
    ws.on_upgrade(  |socket| handle_socket(socket,senderAction ))

    // ws.on_upgrade( |socket| 
    //     {tokio::spawn(
    //          async {handle_socket(socket,senderAction , application_config).await}
     
    //       ).join();
    //      }
 
    // )
}

async fn handle_socket(
    mut socket: WebSocket,
    sender:Sender<SocketAction>
 
) {
    let (TxJSON, RxJSON) = channel(100);
  
    loop {
        tokio::select! {
            val = RxJSON.recv() =>{
                socket.send(RawMessage::Text(val.unwrap())).await.unwrap()
            }
            Some(msg) = socket.recv()=>{
                if let Ok(msg) = msg {
                    match msg {
                        RawMessage::Text(t) => {
                            
                            let actions = processing_requests(&t, sender.clone(),TxJSON.clone(),&socket).await;
                            socket.send(RawMessage::Text("hello world".to_string())).await.unwrap();
                           
                        }
                        RawMessage::Binary(_) => {
                            println!("client sent binary data");
                        }
                        RawMessage::Ping(_) => {
                            println!("socket ping");
                        }
                        RawMessage::Pong(_) => {
                            println!("socket pong");
                        }
                        RawMessage::Close(_) => {
                            println!("client disconnected");
                            return;
                        }
                    }
                } else {
                    println!("client disconnected");
                    return;
                }
            }
        }
        // if let Some(msg) = socket.recv().await {
        //     if let Ok(msg) = msg {
        //         match msg {
        //             RawMessage::Text(t) => {
                            
        //                 let actions = processing_requests(&t, sender.clone(),&socket);
        //                 // socket.send(RawMessage::Text("hello world".to_string())).await.unwrap();
                       
        //             }
        //             RawMessage::Binary(_) => {
        //                 println!("client sent binary data");
        //             }
        //             RawMessage::Ping(_) => {
        //                 println!("socket ping");
        //             }
        //             RawMessage::Pong(_) => {
        //                 println!("socket pong");
        //             }
        //             RawMessage::Close(_) => {
        //                 println!("client disconnected");
        //                 return;
        //             }
        //         }
        //     } else {
        //         println!("client disconnected");
        //         return;
        //     }
        // }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}
pub fn fileNamePartData(params: &Params) -> String{
    // let mut files = Vec::new();
    let exchange = &params.exchange;
    let market_type = &params.market_type;
    let msg_type = &params.msg_type;
    let symbol = &params.symbols;
    let date = &params.date;
    // date+ "/" + exchange+market_type+msg_type+symbol
    // format!("{}/{}_{}_{}_{}", date,exchange, market_type, msg_type, symbol);
    // let mut begin_datetime = Utc.timestamp(params.begin_datetime, 0);
    // let end_datetime = Utc.timestamp(params.end_datetime, 0);
    // let days = (end_datetime - begin_datetime).num_days();
    let fileName = if let Some(period) = &params.period {
        if period.is_empty() {
            format!("{}_{}_{}_{}",exchange, market_type, msg_type, symbol)
        }else {
            format!("{}_{}_{}_{}_{}",exchange, market_type, msg_type, symbol,period)
        }

    } else {
        format!("{}_{}_{}_{}",exchange, market_type, msg_type, symbol)
    };
    fileName

}
#[derive(Deserialize)]
pub struct Params {
    pub exchange: String,
    pub market_type: String,
    pub msg_type: String,
    pub symbols: String,
    pub period: Option<String>,
    // pub begin_datetime: i64,
    // pub end_datetime: i64,
    pub date:String,
    pub day:Option<i64>
}

#[derive(Serialize, Deserialize)]
pub struct Action {
    pub action: String,
    pub params: Value,
    pub echo: Option<i64>,
}
pub struct SocketPerson<'a>{
    pub person:Option<i64>,
    pub websocketId : Option<&'a mut WebSocket>
}
#[derive(Default)]
pub struct ReceiverList{
    pub receiver:Arc<Mutex<HashMap<i64,Sender<String>>>>
}

pub async fn processing_requests(str: &str, sender:Sender<SocketAction>,tx:TxJSON ,socket:&WebSocket) -> String {
    println!("{}",str);
    let params: Action = serde_json::from_str(str).unwrap();
    if let Some(echo) = params.echo {
        let result = String::new();
        let param:Params= serde_json::from_str(&params.params.to_string()).unwrap();
        let fileName = fileNamePartData(&param);
        // let day = if let Some(day) = param.day {
        //      day
        // }else{
        //      0
        // };
        // updateA(&receiverList,&socket);

        //0是单天 1是昨天 2 前天 - 8
        // let r = FileReader::new(fileName, day);

        // for i in r.unwrap() {
        //     println!("{:?}", i);
        //     socket.send(RawMessage::Text(json!(i).to_string())).await.unwrap();
        //     // socket.
        // }
        // let receiver = state.receiver.clone();
        //0是单天 1是昨天 2 前天 - 8
        // let r = FileReader::new("binance_spot_candlestick_BTCUSDT_60".to_string(), 9);
        //
        // for i in r.unwrap() {
        //     println!("{:?}", i);
        // }

            // let locked = receiver.lock();
            // println!("{:?}",locked);
            // let xx = locked.unwrap();
            // println!("{:?}",xx);
            // let receiver = xx.get(&echo).unwrap();
            // println!("22{:?}",receiver);
            // for msg in receiver {
            //     let msg: Message = msg;
            //     println!("1111{}",msg);
            //     let received_at = msg.received_at as i64;
            //
            //
            //     // let orderbook = &orderbook[0];
            //     // result.clone().push_str(&json!(orderbook).to_string());
            //     // break;
            // }

    } else {
        if params.action == "subscribe" {
            //  let mut receivers = receiverList.receiver.lock().unwrap();
            // let mut receiver = state.receiver.lock().unwrap();
            // let (sender,receiver) = channel();
            let mut rng = rand::thread_rng();
            let y = rng.gen::<i64>();
            // let sen=  sender.lock().unwrap();
            sender.send(SocketAction::Add(y,tx));
            // tx.send(Mutex::new((y,socket)));
            // receivers.insert(y, sender);
            return "{\"echo\":".to_string() + &y.to_string() + "}";
        }
        if params.action == "unsubscribe" {
            // let mut receiver = state.receiver.lock().unwrap();
            let echo = params.params["echo"].as_i64().unwrap();
            // receiver.remove(&echo);
            return "{\"echo\":".to_string() + &echo.to_string() + "}";
        }
    }

    return "".to_string();
}