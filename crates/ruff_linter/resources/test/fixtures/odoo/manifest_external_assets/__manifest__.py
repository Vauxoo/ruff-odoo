{
    "name": "My Module",
    "assets": {
        "web.assets_backend": [
            "https://cdn.example.com/lib.js",
            "my_module/static/src/js/widget.js",
            ("replace", "my_module/static/src/js/old.js", "https://cdn.example.com/new.js"),
        ],
    },
}
