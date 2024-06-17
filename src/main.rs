use actix_web::{web, App, HttpResponse, HttpRequest, HttpServer, Responder, http::StatusCode};
use log::{info, warn, debug, LevelFilter};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::collections::HashMap;
use std::fs;
use gethostname::gethostname;

#[derive(Serialize, Deserialize, Clone)]
struct Route {
    method: String,
    path: String,
    response: String,
    code: u16,
    error: bool
}

impl Default for Route {
    fn default() -> Self {
        Self {
            method: "GET".to_string(),
            path: String::new(),
            response: "OK".to_string(),
            code: 200,
            error: false,
        }
    }
}

// Shared state for dynamic routes and server configuration
struct AppState {
    dynamic_routes: Mutex<HashMap<String, Route>>,
    error_percentage: Mutex<u8>,
}

// echo body back
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

// return hostname
async fn host() -> impl Responder {
    // Get the hostname as an OsString
    let os_string = gethostname().into_string().unwrap_or_else(|_| String::from("Unknown Host"));

    // Convert the OsString to a String and use it as the response body
    HttpResponse::Ok().body(os_string)
}

//return OK
async fn healthz() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

// return the status code from the route
async fn status_code(info: web::Path<(u16,)>) -> impl Responder {
    HttpResponse::new(actix_web::http::StatusCode::from_u16(info.0).unwrap())
}

// set response to an error code based on general percentage requested
async fn status_error_percentage(data: web::Data<AppState>) -> impl Responder {
    let error_code = error_code_picker(data).await;
    if error_code != 200 {
        HttpResponse::new(actix_web::http::StatusCode::from_u16(error_code as u16).unwrap())
    } else {
        HttpResponse::Ok().finish()
    }
}

// pick an error code to return
async fn error_code_picker(data: web::Data<AppState>) -> u16 {
    let mut rng = rand::thread_rng();
    let error_percentage = *data.error_percentage.lock().unwrap();
    let random_number = rng.gen_range(0..100);
    if random_number < error_percentage {
        let errors = vec![400, 401, 403, 408, 409, 500, 502, 503, 504]; // List of possible error status codes
        let error_code = errors[rng.gen_range(0..errors.len())];
        return error_code
    } else {
        return 200
    }
}

// Handler for dynamic responses
async fn dynamic_handler(data: web::Data<AppState>, path: web::Path<String>, req: HttpRequest) -> impl Responder {
    debug!("Entering dynamic route handler for: {}", path);
    let path_clone = path.clone(); // Clone the path before moving it
    let route = {
        let routes = data.dynamic_routes.lock().unwrap();
        routes.get(&*path_clone).cloned()
    };

    if let Some(route) = route {
        debug!("Handling dynamic route: {} {}", route.method, route.path);
        let status_code = StatusCode::from_u16(route.code).unwrap_or(StatusCode::OK);

        // Check if the request method matches the defined method
        if *req.method() == *route.method {
            // If route is set to intentionally error then pass off to error response handler
            if route.error {
                let error_code = error_code_picker(data).await;
                let status_code = StatusCode::from_u16(error_code).unwrap_or(StatusCode::OK);
                return HttpResponse::build(status_code).finish();
            } else {
                match route.method.as_str() {
                    "GET" | "POST" | "PATCH" | "PUT" => HttpResponse::build(status_code).body(route.response),
                    "DELETE" => HttpResponse::build(status_code).finish(),
                    _ => HttpResponse::NotFound().finish(), // assume a 404 route does not match dynamic routes
                }
            }
        } else {
            warn!("Method mismatch for path: {}", path.into_inner());
            HttpResponse::MethodNotAllowed().finish()
        }
    } else {
        warn!("Route not found for path: {}", path.into_inner());
        HttpResponse::NotFound().finish()
    }
}

// Handler to update dynamic routes
async fn update_routes(req_body: String, data: web::Data<AppState>) -> impl Responder {
    info!("Adding new route: {}", req_body);
    let new_routes: Vec<Route> = serde_json::from_str(&req_body).unwrap_or_default();
    let mut routes = data.dynamic_routes.lock().unwrap();
    for route in new_routes {
        routes.insert(route.path.clone(), route);
    }
    HttpResponse::Ok().finish()
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize the logger
    env_logger::builder()
    .filter_level(LevelFilter::Info)
    .init();

    // Read the initial routes and configuration from a YAML file
    let config: HashMap<String, serde_yaml::Value> = match fs::read_to_string("routes.yml") {
        Ok(content) => match serde_yaml::from_str(&content) {
            Ok(yaml) => yaml,
            Err(_) => panic!("Failed to parse YAML from routes.yaml"),
        },
        Err(_) => panic!("Failed to read routes.yml"),
    };

    // Parse dynamic routes
    let dynamic_routes: HashMap<String, Route> = if let Some(routes_value) = config.get("routes") {
        debug!("Loading config: {:?}", config);
        match serde_yaml::from_value(routes_value.clone()) {
            Ok(routes) => routes,
            Err(_) => panic!("Failed to parse routes from routes.yml"),
        }
    } else {
        panic!("Failed to load routes from routes.yml");
    };

    
    // Handle error percentage
    let error_percentage = if let Some(error_value) = config.get("error_percentage") {
        error_value.as_str().and_then(|e| e.parse().ok()).unwrap_or(0)
    } else {
        0
    };

    // Log the loaded routes from json
    for (path, route) in &dynamic_routes {
        info!("Loaded route: {} {} -> {}", route.method, path, route.response);
    }

    info!("Setting error percentage to {}", error_percentage);

    let app_data = web::Data::new(AppState {
        dynamic_routes: Mutex::new(dynamic_routes),
        error_percentage: Mutex::new(error_percentage),
    });

    // Get the server port from the configuration or default to 8080
    let server_port = config.get("port").and_then(|v| v.as_str()).unwrap_or("8080").to_string();

    let hostname = config.get("hostname").and_then(|v| v.as_str()).unwrap_or("0.0.0.0").to_string();

    // Start the server in a new thread
    let server = HttpServer::new(move || {
        App::new()
        .wrap(actix_web::middleware::Logger::default())    
        .app_data(app_data.clone())
            .service(web::resource("/echo").route(web::post().to(echo)))
            .service(web::resource("/host").route(web::get().to(host)))
            .service(web::resource("/healthz").route(web::get().to(healthz)))
            .service(web::resource("/status/{code}").route(web::get().to(status_code)).route(web::post().to(status_code)))
            .service(web::resource("/errors").route(web::get().to(status_error_percentage)))
            .service(web::resource("/routes").route(web::put().to(update_routes)).route(web::post().to(update_routes)))
            .service(web::resource("/{path:.*}")
                .route(web::get().to(dynamic_handler))
                .route(web::post().to(dynamic_handler))
                .route(web::put().to(dynamic_handler))
                .route(web::delete().to(dynamic_handler))
                .route(web::patch().to(dynamic_handler))
            )
    })
    .bind(format!("{}:{}", hostname, server_port))?
    .run();
    
    info!("Listening on {}:{}", hostname, server_port);

    server.await
}
