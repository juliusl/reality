    ```runmd
    + .operation test_std_io                                # Tests std io utilities
    <utility/loopio.ext.std.io>
    <..println>             Hello World                     # Prints a new line
    <..read_text_file>      loopio/examples/test.txt        # Read a text file into transient storage
    <test>                  Hello World 2                   # Verifies the file

    + .operation test_hyper                                  # Tests hyper utilities
    <echo>                                                   # Echoes an incoming request, Also schedules a shutdown
    <utility/loopio>                                         # Enable utilities
    <..hyper.request> testhost://test-engine-proxy/test      # Send outbound request

    + .operation test_process
    <utility/loopio.ext.std.process>    ls
    : .arg -la

    + .operation test_poem                                   # Tests poem utilities
    <utility/loopio>
    <..poem.engine-proxy> localhost:0                        # Runs a local server that can start operations or sequences
    : .alias testhost://test-engine-proxy
    : test          .route test_std_io
    : test_2        .route run_println
    : test_handler  .route test_hyper
    : test_3        .route test_process
    : test          .get /test
    : test_2        .get /test2
    : test_3        .get /test3
    : test_handler  .get /test-handler/:name

    + .operation test_poem_reverse_proxy
    <utility/loopio>
    <..receive-signal>              engine_proxy_started
    : .host                         testhost
    <..poem.reverse-proxy-config>   testhost://test-engine-proxy
    <..poem.reverse-proxy>          localhost:3576
    : .host                         testhost://test-engine-proxy

    + .sequence start_tests                                  # Sequence that starts the demo
    : .next test_std_io
    : .next test_poem_reverse_proxy, test_poem
    : .loop false

    + .sequence run_println                                  # Sequence that can be called by the engine proxy
    : .next test_std_io
    : .loop false

    + .host testhost                                         # Host configured w/ a starting sequence
    : .start        start_tests
    : .condition    engine_proxy_started

    ```