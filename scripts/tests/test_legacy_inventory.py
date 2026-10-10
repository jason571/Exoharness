import importlib.util
from pathlib import Path
import unittest

SPEC=importlib.util.spec_from_file_location("inventory",Path(__file__).resolve().parents[1]/"audit-legacy-api.py")
audit=importlib.util.module_from_spec(SPEC);SPEC.loader.exec_module(audit)


class InventoryTests(unittest.TestCase):
    def test_methods_multiline_routes_surfaces_and_source_lines_are_preserved(self):
        source='''pub fn app(state: State) -> Router {
            Router::new().route("/v1/items", get(list).post(create))
                .route("/v1/items/{id}", axum::routing::delete(remove))
                .route("/assets/example.js", get(|| async { "}); not Rust" }))
        }
        pub fn portal_app(state: State) -> Router {
            // .route("/fake", get(fake))
            Router::new().route(
                "/v1/items", get(own_items).put(update)
            )
        }
        fn test() { let route = ".route(escaped)"; }
        '''
        entries=audit.inventory(source)
        self.assertEqual({(item["surface"],item["method"],item["path"]) for item in entries}, {
            ("management","GET","/v1/items"),("management","POST","/v1/items"),
            ("management","DELETE","/v1/items/{id}"),("management","GET","/assets/example.js"),
            ("portal","GET","/v1/items"),("portal","PUT","/v1/items")})
        self.assertTrue(all(item["acceptance"]=="pending" and not item["evidence"] for item in entries))
        self.assertTrue(all(item["path"]!="/fake" for item in entries))

    def test_missing_duplicate_and_unknown_router_methods_fail_closed(self):
        for source in ["pub fn app() {}",'pub fn app(){x.route("/x",opaque(handler))} pub fn portal_app(){}',
            'pub fn app(){x.route("/x",get(a)).route("/x",get(b))} pub fn portal_app(){}']:
            with self.assertRaises(ValueError):audit.inventory(source)
