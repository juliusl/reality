    ```runmd
    + .operation test_std_io                                            # Tests std io utilities
    <store/loopio.stdio.println>          Hello World                                 # Prints a new line
    | abc
    |   def
    |     ghi
    <..io.read-text-file>   loopio/examples/test.txt                    # Read a text file into transient storage
    <user.test>             Hello World 2                               # Verifies the file

    + .operation test_hyper                                             # Tests hyper utilities
    <user.echo>                                                         # Echoes an incoming request, Also schedules a shutdown
    <store/loopio.hyper.request>  testhost://test-engine-proxy/test     # Enable utilities

    + .operation test_process
    <store/loopio.std.process>    ls
    : .arg -la
    : .piped true
    <user.test>

    + .operation test_poem                                      # Tests poem utilities
    <loopio.poem.engine-proxy> localhost:0                      # Runs a local server that can start operations or sequences
    |# notify = teshost://engine_proxy_started
    
    : .alias testhost://test-engine-proxy
    : test          .route test_std_io
    : test_2        .route run_println
    : test_handler  .route test_hyper
    : test_3        .route test_process
    : test          .path /test
    : test_2        .path /test2
    : test_3        .path /test3
    : test_handler  .path /test-handler/:name

    + .operation start_reverse_proxy
    <loopio.receive-signal>                     engine_proxy_started
    : .host                                     testhost
    <loopio.poem.reverse-proxy-config>          testhost://test-engine-proxy
    <loopio.poem.reverse-proxy>                 localhost:3576
    : .host                                     testhost://test-engine-proxy

    + .sequence start_tests                                  # Sequence that starts the demo
    : .step test_std_io
    |# kind = once
    
    : .step start_reverse_proxy, test_poem
    : .loop false

    + .sequence run_println                                  # Sequence that can be called by the engine proxy
    : .step test_std_io
    : .loop false

    + .host testhost                                         # Host configured w/ a starting sequence
    : .start        start_tests
    : .condition    engine_proxy_started
    ```