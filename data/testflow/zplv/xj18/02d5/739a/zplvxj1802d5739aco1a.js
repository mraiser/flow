var me = this; 
var ME = $('#'+me.UUID)[0];

me.ready = function(){
  me.testSync("websock", function(){
    var hold = SOCK;
    SOCK = null;
	me.testSync("ajax", function(){
      SOCK = hold;
      me.testAsync();
    });
  });
};

me.testAsync = function(){
  var el = $(ME).find(".async");
  el.append("----------------- async -------------------------<br>");
  send_test_add(299,121, function(result) {
    el.append("test_add: "+JSON.stringify(result)+"<BR>");
  });
  send_test_command(210, function(result) {
    el.append("test_command: "+JSON.stringify(result)+"<BR>");
  });
  send_test_conditionals(true, function(result) {
    el.append("test_conditionals:"+JSON.stringify(result)+"<BR>");
  });
  send_test_lists([1,2,3], function(result) {
    el.append("test_lists:"+JSON.stringify(result)+"<BR>");
  });
  send_test_loop(0, function(result) {
    el.append("test_loop:"+JSON.stringify(result)+"<BR>");
  });
  send_test_speed(10000, function(result) {
    el.append("test_speed:"+JSON.stringify(result)+"<BR>");
  });
  send_test_rust("world", function(result) {
    el.append("test_rust: "+JSON.stringify(result)+"<BR>");
  });
  send_test_java("world", function(result) {
    el.append("test_java: "+JSON.stringify(result)+"<BR>");
  });
  send_test_javascript("world", function(result) {
    el.append("test_javascript: "+JSON.stringify(result)+"<BR>");
  });
  send_test_python("world", function(result) {
    el.append("test_python: "+JSON.stringify(result)+"<BR>");
  });
};

me.testSync = function(claz, cb){
  var el = $(ME).find("."+claz);
  el.append("----------------- "+claz+" -------------------------<br>");
  send_test_add(299,121, function(result) {
    el.append("test_add: "+JSON.stringify(result)+"<BR>");
    send_test_command(210, function(result) {
      el.append("test_command: "+JSON.stringify(result)+"<BR>");
      send_test_conditionals(true, function(result) {
        el.append("test_conditionals:"+JSON.stringify(result)+"<BR>");
        send_test_lists([1,2,3], function(result) {
          el.append("test_lists:"+JSON.stringify(result)+"<BR>");
          send_test_loop(0, function(result) {
            el.append("test_loop:"+JSON.stringify(result)+"<BR>");
            send_test_speed(10000, function(result) {
              el.append("test_speed:"+JSON.stringify(result)+"<BR>");
              send_test_rust("world", function(result) {
                el.append("test_rust: "+JSON.stringify(result)+"<BR>");
                send_test_java("world", function(result) {
                  el.append("test_java: "+JSON.stringify(result)+"<BR>");
                  send_test_javascript("world", function(result) {
                    el.append("test_javascript: "+JSON.stringify(result)+"<BR>");
                    send_test_python("world", function(result) {
                      el.append("test_python: "+JSON.stringify(result)+"<BR>");
                      cb();
                    });
                  });
                });
              });
            });
          });
        });
      });
    });
  });
};

