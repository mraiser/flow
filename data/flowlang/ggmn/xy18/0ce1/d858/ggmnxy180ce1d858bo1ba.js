var me = this; 
var ME = $('#'+me.UUID)[0];

me.ready = function(){
  var el = $(ME).find("div");
  send_unique_session_id(function(result) {
    el.append("unique_session_id: "+JSON.stringify(result)+"<BR>");
  });
};
