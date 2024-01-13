    ```runmd
    + .operation a
    <builtin.println>       Hello World

    + .operation test_std_io                                            # Tests std io utilities
    <store/builtin.println>          Hello World                   # Prints a new line
    | abc
    |   def
    |     ghi
    <builtin.read-text-file>   loopio/examples/test.txt                    # Read a text file into transient storage
    <user.test>             Hello World 2                               # Verifies the file

    + .operation test_hyper                                             # Tests hyper utilities
    <user.echo>                                                         # Echoes an incoming request, Also schedules a shutdown
    <store/builtin.request>  testhost://start_engine_proxy/test     # Enable utilities

    + .operation test_process
    <store/builtin.process>    ls
    : .arg -la
    : .piped true
    <user.test>

    + .operation start_engine_proxy                                      # Tests poem utilities
    <builtin.engine-proxy> localhost:0                              # Runs a local server that can start operations or sequences
    |# notify = test-engine-proxy

    : .alias testhost://start_engine_proxy
    
    : test          .route test_std_io
    : test_2        .route run_println
    : test_handler  .route test_hyper
    : test_3        .route test_process

    : test          .path /test
    : test_2        .path /test2
    : test_3        .path /test3
    : test_handler  .path /test-handler/:name

    + .operation start_reverse_proxy
    <builtin.reverse-proxy-config>  testhost://start_engine_proxy
    |# listen = test-engine-proxy
    
    <builtin.reverse-proxy>         localhost:3576
    : .forward testhost://start_engine_proxy

    + .sequence start_tests                                             # Sequence that starts the demo
    : .step test_std_io
    |# kind = once
    
    : .step testhost://start_engine_proxy, testhost://start_reverse_proxy
    : .loop false

    + .sequence run_println                                             # Sequence that can be called by the engine proxy
    : .step test_std_io
    : .loop false

    + .host testhost                                                    # Host configured w/ a starting sequence
    : .start        start_tests
    : .action       start_reverse_proxy
    : .action       start_engine_proxy
    : .event        test-engine-proxy
    

    ```