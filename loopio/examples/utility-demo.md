    ```runmd
    + .operation a
    <builtin.println>       Hello World

    # -- Tests std io utilities
    + .operation test_std_io

    # -- Prints a new line
    <store/builtin.println>          Hello World
    | abc
    |   def
    |     ghi

    # -- Read a text file into transient storage
    <builtin.read-text-file>   loopio/examples/test.txt

    # -- Verifies the file
    <user.test>             Hello World 2

    # -- Tests hyper utilities
    + .operation test_hyper

    # -- Echoes an incoming request, Also schedules a shutdown
    <user.echo>

    # -- Enable utilities
    <store/builtin.request>  testhost://start_engine_proxy/test     

    + .operation test_process
    <store/builtin.process>    ls
    : .arg -la
    : .piped true
    <user.test>

    # -- Tests poem utilities
    + .operation start_engine_proxy

    # -- Runs a local server that can start operations or sequences
    <builtin.engine-proxy> localhost:0
    |# notify = test-engine-proxy

    : .alias testhost://start_engine_proxy
    
    # -- Route /test
    : .route test_std_io
    |# path = /test

    # -- Route /test2
    : .route run_println
    |# path = /test2

    # -- Route /test2
    : .route test_hyper
    |# path = /test3

    # -- Route /test2
    : .route test_process
    |# path = /test-handler/:name

    + .operation start_reverse_proxy
    <builtin.reverse-proxy-config>
    |# listen = test-engine-proxy
    
    <builtin.reverse-proxy>         localhost:3576
    : .forward testhost://start_engine_proxy

    # -- Sequence that starts the demo
    + .sequence start_tests
    : .step test_std_io
    |# kind = once
    
    : .step start_engine_proxy, start_reverse_proxy
    : .loop false

    # -- Sequence that can be called by the engine proxy
    + .sequence run_println
    : .step test_std_io
    : .loop false

    # -- Host configured w/ a starting sequence
    + .host testhost
    : .start        start_tests
    : .action       start_reverse_proxy
    : .action       start_engine_proxy
    : .event        test-engine-proxy
    

    ```